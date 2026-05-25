# How to integrate a Go job board with jobstack-bpp

A practical guide for wiring an existing Go job board (entities: `Organisation`, `User`, `Job`, `Application`) into this BPP so its jobs become discoverable on the Beckn / ONDC network.

---

## 1. What you're building

`jobstack-bpp` is a thin Beckn protocol adapter. It receives Beckn webhooks (`search`, `select`, `init`, `confirm`, `status`), forwards them to a "provider DB" over HTTP, and POSTs the response back to the Beckn caller as `on_*`. It does **not** persist anything.

Your Go job board becomes that provider DB. You expose **5 new HTTP endpoints under `/beckn/*`** that translate between Beckn shapes and your internal domain. Everything else (signing, registry, BAP routing) is handled by a separate **BPP-Adapter** that sits in front of `jobstack-bpp`.

```
                       ┌─────────────────────────────────────────────┐
                       │                Beckn Network                │
                       │   ┌────────┐        ┌─────────┐             │
   Candidate ─────────▶│   │  BAP   │ ◀────▶ │ Gateway │             │
   (job seeker)        │   └────┬───┘        └────┬────┘             │
                       └────────┼─────────────────┼─────────────────-┘
                                ▼                 ▼
                       ┌──────────────────────────────────┐
                       │  BPP-Adapter (beckn-onix / etc.) │
                       │  signs / verifies / routes       │
                       └────────────────┬─────────────────┘
                                        ▼
                       ┌──────────────────────────────────┐
                       │     jobstack-bpp (Rust, :3009)   │
                       │  /webhook/{action} → ACK → spawn │
                       │  POST {db_uri}/beckn/{action}    │
                       │  POST {caller_uri}/on_{action}   │
                       └────────────────┬─────────────────┘
                                        │ POST /beckn/{action}
                                        ▼
   ┌────────────────────────────────────────────────────────────────┐
   │   YOUR Go Job Board                                            │
   │   Existing CRUD  +  NEW /beckn/* adapter handlers              │
   │   ────────────────  ────────────────────────────────           │
   │   /organisations    /beckn/search                              │
   │   /users            /beckn/select                              │
   │   /jobs             /beckn/init                                │
   │   /applications     /beckn/confirm                             │
   │                     /beckn/status                              │
   │                                                                │
   │   Postgres: organisations, users, jobs, applications,          │
   │             beckn_messages (NEW)                               │
   └────────────────────────────────────────────────────────────────┘
```

---

## 2. The 5 endpoints to add

All under `/beckn/*`. Content-Type is `application/json`. Return HTTP 2xx with `{ "message": {...}, "pagination"?: {...} }`. Anything outside `message` and `pagination` is discarded by the BPP — it rebuilds the Beckn `context` itself.

### Request shapes the BPP sends

| Endpoint | Wrapped body (sent by BPP) | Source |
|---|---|---|
| `POST /beckn/search`  | `{ "message": <intent>, "pagination": { "page": 0, "limit": 50 }, "options"?: ... }` | `src/services/search.rs:13-30` |
| `POST /beckn/select`  | `{ "message": <order with item refs> }` | `src/services/select.rs:14-17` |
| `POST /beckn/init`    | `{ "message": <order draft>, "context": <Beckn ctx> }` | `src/services/init.rs:14-18` |
| `POST /beckn/confirm` | `{ "message": <order>, "context": <Beckn ctx> }` | `src/services/confirm.rs:14-18` |
| `POST /beckn/status`  | `{ "message": { "order_id": "..." }, "context": <Beckn ctx> }` | `src/services/status.rs:14-18` |

The `context` field carries `domain`, `action`, `bap_id`, `bap_uri`, `transaction_id`, `message_id`, `timestamp`, `ttl`, `version`. Use `transaction_id` + `message_id` for idempotency and audit.

### Response shape you return

```json
{
  "message": { /* action-specific payload — catalog for search, order for the rest */ },
  "pagination": { "page": 0, "limit": 50, "total": 1234 }   // search only, optional
}
```

If you 5xx or return non-JSON, the BAP gets the ACK but **never gets a callback** — there's no retry layer.

---

## 3. Domain mapping

### Organisation → `provider`

```go
beckn.Provider{
    ID:         org.ExternalID,            // URN, e.g. "org/{uuid}@yourjobboard.com"
    Descriptor: beckn.Descriptor{Name: org.Name},
    Locations:  mapLocations(org.Locations),
}
```

### Job → `item`

The hardest mapping. Use the Jobs tag groups from `src/utils/mock_responses/response.status.json`:

| Your `Job` field | Beckn target |
|---|---|
| `title` | `item.descriptor.name` |
| `description` | `item.descriptor.long_desc` |
| `posted_at` / `closes_at` | `item.time.range.start` / `end` |
| `salary_min` / `salary_max` | `tags[code=salary-info].list[]` → `gross-min`, `gross-max` |
| `experience_years_required` | `tags[code=job-requirements].list[]` → `req-experience` |
| `skills_required[]` | `tags[code=job-requirements].list[]` → `req-prof-skill` (one entry per skill) |
| `responsibilities[]` | `tags[code=job-responsibilities].list[]` → `responsibility` |
| `education_requirements[]` | `tags[code=academic-eligibility].list[]` → `course-name` / `course-level`, `min-percentage`, `mandatory-eligibility` |
| `industry` / `department` / `employment_type` / `role` | `tags[code=listing-details].list[]` |
| `location` | `item.location_ids[]` + `provider.locations[]` |

### User (candidate) → `customer.person`

```go
beckn.Customer{
    Person: beckn.Person{
        ID:        user.ExternalID,        // BAP-supplied person.id if present, else your own URN
        Name:      user.Name,
        Skills:    mapSkills(user.Skills),
        Languages: mapLanguages(user.Languages),
        Tags: []beckn.Tag{
            {Code: "emp-details", List: []beckn.TagItem{
                {Code: "expected-salary",   Value: user.ExpectedSalary},
                {Code: "total-experience",  Value: user.TotalExperience},
            }},
            {Code: "documents", List: []beckn.TagItem{
                {Code: "doc-type",    Value: "resume"},
                {Code: "link",        Value: user.ResumeURL},
                {Code: "file-format", Value: "pdf"},
            }},
        },
    },
    Contact: beckn.Contact{Phone: user.Phone, Email: user.Email},
}
```

### Application → `order`

`order.id == application.external_id`. `order.fulfillments[].state.descriptor.code` carries current status.

### Application.Status → fulfillment state

| Go `Application.Status` | Beckn `fulfillment.state.descriptor.code` |
|---|---|
| `draft` | `INITIATED` |
| `applied` | `APPLIED` |
| `shortlisted` | `SHORTLISTED` |
| `interview_scheduled` | `INTERVIEW-SCHEDULED` |
| `offer_extended` | `OFFER-EXTENDED` |
| `offer_accepted` / `hired` | `HIRED` |
| `rejected` | `REJECTED` |
| `withdrawn` | `CANCELLED` |

---

## 4. Suggested Go package layout

Add three packages alongside your existing code:

```
beckn/                           # pure DTOs — no DB, no IO
  context.go
  catalog.go                     # Catalog, Provider, Item, Descriptor, Price, Location
  order.go                       # Order, Quote, Breakup, Fulfillment, Customer, Person, Billing, Payment
  tags.go                        # Tag, TagItem + Jobs tag-group code constants
  request.go                     # SearchRequest, SelectRequest, InitRequest, ConfirmRequest, StatusRequest
  response.go                    # MessageEnvelope { Message any; Pagination *Pagination }

internal/becknmap/               # pure mapper functions
  job_to_item.go
  org_to_provider.go
  user_to_customer.go
  application_to_order.go
  intent_to_filter.go            # parse Beckn search intent → your job-filter struct
  order_to_application.go        # init/confirm body → draft Application + User upsert
  status_mapping.go              # the table above as code

internal/http/beckn/             # HTTP handlers
  middleware.go                  # idempotency, shared-secret auth, logging
  search.go
  select.go
  init.go
  confirm.go
  status.go
  router.go                      # wires the 5 handlers
```

Keep `internal/becknmap` pure — no DB, no logger, easy to unit-test with golden JSON fixtures copied from `src/utils/mock_responses/`.

---

## 5. Schema additions

### `jobs`
Add if missing — needed by the Beckn Jobs tag groups:

```sql
ALTER TABLE jobs
  ADD COLUMN external_id            text UNIQUE NOT NULL,  -- URN, goes into item.id
  ADD COLUMN experience_years_required  numeric,
  ADD COLUMN skills_required        text[],
  ADD COLUMN responsibilities       text[],
  ADD COLUMN industry               text,
  ADD COLUMN department             text,
  ADD COLUMN employment_type        text,
  ADD COLUMN role                   text,
  ADD COLUMN education_requirements jsonb,                 -- list of {course, level, min_percentage, mandatory}
  ADD COLUMN salary_min             numeric,
  ADD COLUMN salary_max             numeric,
  ADD COLUMN salary_currency        text DEFAULT 'INR',
  ADD COLUMN posted_at              timestamptz,
  ADD COLUMN closes_at              timestamptz,
  ADD COLUMN status                 text DEFAULT 'open';   -- open / closed / paused
```

### `organisations`
```sql
ALTER TABLE organisations
  ADD COLUMN external_id        text UNIQUE NOT NULL,      -- URN, goes into provider.id
  ADD COLUMN expose_on_network  boolean DEFAULT false;     -- opt-in flag
```

### `users` (candidates)
BAP-sourced candidates won't have a password / email-verification flow. Make them sparse:

```sql
ALTER TABLE users
  ADD COLUMN external_id text UNIQUE,                      -- BAP-supplied person.id, used for dedup
  ADD COLUMN source      text DEFAULT 'direct';            -- 'direct' | 'beckn:<bap_id>'

ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;
```

### `applications`
```sql
ALTER TABLE applications
  ADD COLUMN external_id           text UNIQUE NOT NULL,    -- URN, becomes order.id on the network
  ADD COLUMN source                text DEFAULT 'direct',   -- 'direct' | 'beckn'
  ADD COLUMN bap_id                text,
  ADD COLUMN bap_uri               text,
  ADD COLUMN beckn_transaction_id  text,
  ADD COLUMN beckn_message_id      text;

CREATE UNIQUE INDEX applications_beckn_txn_uniq
  ON applications (beckn_transaction_id)
  WHERE beckn_transaction_id IS NOT NULL;
```

### NEW table — `beckn_messages` (audit + idempotency)

```sql
CREATE TABLE beckn_messages (
  id              bigserial PRIMARY KEY,
  transaction_id  text   NOT NULL,
  message_id      text   NOT NULL,
  action          text   NOT NULL,                          -- search/select/init/confirm/status
  direction       text   NOT NULL,                          -- 'inbound' | 'outbound'
  bap_id          text,
  bpp_id          text,
  request_body    jsonb,
  response_body   jsonb,
  status          text   NOT NULL,                          -- 'in_progress' | 'ok' | 'error'
  error           text,
  received_at     timestamptz NOT NULL DEFAULT now(),
  completed_at    timestamptz
);

CREATE UNIQUE INDEX beckn_messages_idem
  ON beckn_messages (transaction_id, action)
  WHERE direction = 'inbound';
```

---

## 6. Idempotency middleware

Key on `(context.transaction_id, context.action)`:

- **Writes** (`init`, `confirm`): take a unique-insert on `beckn_messages` with `status='in_progress'` as your lock. If insert fails because a row already exists:
  - if existing row is `ok`, return its `response_body` verbatim.
  - if `in_progress`, return `409` (or wait briefly).
  - if `error`, return the stored error or retry — your call.
  Do the work, then `UPDATE beckn_messages SET status='ok', response_body=$1, completed_at=now() WHERE id=$2`.
- **Reads** (`search`, `select`, `status`): no dedup needed; just write an audit row.

Sketch:

```go
func IdempotencyMiddleware(db *sql.DB) func(http.Handler) http.Handler {
    return func(next http.Handler) http.Handler {
        return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
            var body struct {
                Context beckn.Context   `json:"context"`
                Message json.RawMessage `json:"message"`
            }
            raw, _ := io.ReadAll(r.Body)
            _ = json.Unmarshal(raw, &body)

            // restore body for downstream handler
            r.Body = io.NopCloser(bytes.NewReader(raw))

            action := actionFromPath(r.URL.Path)
            isWrite := action == "init" || action == "confirm"

            if isWrite {
                if cached, ok := tryLockOrFetch(db, body.Context, action, raw); ok {
                    w.Header().Set("Content-Type", "application/json")
                    _, _ = w.Write(cached)
                    return
                }
            }

            // capture response so we can persist it
            rec := httptest.NewRecorder()
            next.ServeHTTP(rec, r)
            persistAudit(db, body.Context, action, raw, rec.Body.Bytes(), rec.Code)
            copyTo(w, rec)
        })
    }
}
```

---

## 7. Search query builder

Parse `body.message.intent`:

| Beckn intent path | Your filter |
|---|---|
| `intent.item.descriptor.name` | keyword (ILIKE on title/description) |
| `intent.fulfillment.stops[].location.city.code` | city |
| `intent.item.category_ids` / `intent.category.id` | category |
| `intent.item.tags[code=salary-info].list[].value` | salary band (`gross-min`, `gross-max`) |
| `intent.item.tags[code=job-requirements].list[].value` (`req-prof-skill`) | required skills (array overlap) |
| `intent.item.tags[code=job-requirements].list[].value` (`req-experience`) | min experience |
| `body.pagination.page` / `limit` | SQL OFFSET / LIMIT |

Return total separately so the response can populate `pagination.total`.

---

## 8. Auth between BPP and Go

The BPP sends bare POSTs (`src/utils/http_client.rs:6-8` — no headers). Choose one:

1. **Private network** (k8s service-to-service / VPC) — do nothing in Go.
2. **Shared secret** (recommended) — middleware on `/beckn/*` requires `X-BPP-Secret: <token>`. Needs a one-line BPP patch in `src/utils/http_client.rs` to attach the header.
3. **mTLS** — heavier; only if you must.

For #2:

```go
func RequireSharedSecret(secret string) func(http.Handler) http.Handler {
    return func(next http.Handler) http.Handler {
        return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
            if subtle.ConstantTimeCompare([]byte(r.Header.Get("X-BPP-Secret")), []byte(secret)) != 1 {
                http.Error(w, "unauthorized", http.StatusUnauthorized)
                return
            }
            next.ServeHTTP(w, r)
        })
    }
}
```

---

## 9. Outbound notifications back to the BAP — open design decision

This BPP only sends `on_*` in direct response to an inbound webhook (`src/services/webhook.rs:13-58`). There is **no path today** for your Go side to push application-status changes to the BAP. When a recruiter changes an application's state in your Go UI, the BAP only learns by polling `status`.

Three options:

- **A. Polling** — accept it; BAP polls `/beckn/status`. No changes needed. Stale UX for the candidate.
- **B. Add an unsolicited push endpoint to the BPP** — small BPP change: new `POST /push/status` that Go calls when an application changes state; BPP rebuilds the Beckn context using stored `transaction_id`/`bap_id`/`bap_uri` (you'd have those in `beckn_messages`) and POSTs to the adapter as `on_status`.
- **C. Implement Beckn `update`** — proper spec route; more work; requires adding the action to `services/webhook.rs` and a matching handler.

**Park this decision** until you have a BAP partner; their preference shapes the choice.

---

## 10. Wiring & deployment

### BPP config (`config/local.yaml` in this repo)

```yaml
debug: true
use_mock_bpp_response: false             # flip off mocks for real integration

bpp:
  id: "bpp.yourjobboard.com"             # must be registered with Beckn Registry
  caller_uri: "http://bpp-adapter:8080"  # ← the BPP-Adapter's inbound for on_* callbacks
                                          #    NOT a BAP URL, NOT the gateway URL
  domain: "ONDC:TRV10"                   # Jobs domain
  version: "2.0.0"
  ttl: "PT30S"

http:
  address: 0.0.0.0
  port: 3009

provider_db:
  db_uri: "http://job-board:8080"        # your Go app; BPP will append /beckn/{action}
```

### Run

```sh
# from this repo
cargo run -- config/local.yaml
# or
docker compose up
```

### Production prerequisites

- A **BPP-Adapter** in front of jobstack-bpp (e.g., [beckn-onix](https://github.com/beckn/beckn-onix) BPP adapter, or Dhiway's adapter referenced in the mocks). It owns the Ed25519 key pair, verifies inbound signatures, and signs outbound `on_*`.
- Your `bpp_id` + the adapter's public HTTPS URL + the BPP's Ed25519 public key registered with the Beckn Registry for your network.
- TLS termination in front of the adapter.

### What you do NOT add to Go

- Ed25519 signing or verification — BPP-Adapter does it.
- Beckn Registry lookups — BPP-Adapter does it.
- Routing `on_*` to specific BAPs — BPP-Adapter reads `context.bap_uri`.
- Gateway broadcast / fan-out handling — Gateway does it.

---

## 11. Tests

- **Unit**: each `becknmap.*` function with golden JSON fixtures copied from `src/utils/mock_responses/*.json`. These are committed and become your contract.
- **Integration**: spin up Go + Postgres in `docker-compose`, POST the BPP-wrapped fixtures to `/beckn/*`, assert response JSON shape + DB state (Application row created, status correct).
- **End-to-end** (optional): bring up BPP + Go + a request-bin as the BAP-Adapter. POST `/webhook/search` to the BPP, assert an `on_search` lands at the bin with the expected catalog.

---

## 12. Minimum viable slice — suggested build order

If you want to ship something callable end-to-end fast:

1. Add the `beckn` DTO package + `job_to_item.go` + `intent_to_filter.go`.
2. Implement `POST /beckn/search` only, plus the `beckn_messages` audit table.
3. Point BPP at it (`provider_db.db_uri`). Keep mocks on for the other 4 actions (`use_mock_bpp_response: true` won't work selectively, so either set it false and accept 5xx for unimplemented actions, or temporarily stub them in Go to return the bundled mocks).
4. Get a real BAP (or a curl harness mimicking one) to call `/webhook/search`. Verify catalog renders end-to-end.
5. Add `select` → `init` → `confirm` → `status` and the Application write path.
6. Add idempotency middleware once you have writes.
7. Decide on the outbound-status strategy (section 9) with your BAP partner.

---

## Quick reference — BPP source files

| Concern | File |
|---|---|
| Inbound webhook route | `src/http/routes/routes.rs:24` |
| Action dispatcher | `src/services/webhook.rs:13-58` |
| Per-action handlers (wrapping shape) | `src/services/{search,select,init,confirm,status}.rs` |
| Outbound HTTP (BPP → your Go app) | `src/utils/http_client.rs:6-27` |
| Provider DB & BAP-callback URL builders | `src/utils/shared.rs` |
| Response context generation | `src/utils/payload_generator.rs:7-43` |
| Mock payloads (use as fixtures) | `src/utils/mock_responses/*.json` |
| Config schema | `src/config.rs`, `config/example.yaml` |
