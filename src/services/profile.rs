use crate::db::profile::{
    delete_stale_profiles, fetch_profiles, get_candidate_by_id, store_profiles,
    MarketInsightsParams, NewProfile, TalentSearchParams,
};
use crate::models::profiles::ProfileSearchRequest;
use crate::models::search::{
    Intent, LocationDistribution, MarketInsightsRequest as ModelMarketInsightsRequest, Pagination,
    SearchMessage, TalentSearchRequest as ModelTalentSearchRequest,
};
use crate::models::webhook::{Ack, AckResponse, AckStatus, WebhookPayload};
use crate::state::AppState;
use crate::utils::http_client::post_json;
use crate::utils::payload_generator::build_profile_beckn_request;

use axum::{extract::Path, extract::State, http::StatusCode, Json};
use chrono::Utc;
use deadpool_redis::redis::AsyncCommands;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

fn hash_json(value: &Value) -> String {
    let canonical = serde_json::to_vec(value).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(canonical);
    format!("{:x}", hasher.finalize())
}

fn extract_profiles_from_on_search(payload: &WebhookPayload, txn_id: &str) -> Vec<NewProfile> {
    let mut profiles = Vec::new();

    let providers = payload
        .message
        .get("catalog")
        .and_then(|c| c.get("providers"))
        .and_then(|p| p.as_array())
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    for provider in providers {
        let items = provider
            .get("items")
            .and_then(|i| i.as_array())
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        for item in items {
            let profile_id = match item.get("id").and_then(|v| v.as_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };

            let beckn_structure = item.clone();
            let hash = hash_json(&beckn_structure);

            profiles.push(NewProfile {
                profile_id,
                beckn_structure: Some(beckn_structure),
                metadata: None,
                hash,
                last_synced_at: Some(Utc::now()),
                transaction_id: txn_id.to_string(),
                bpp_id: payload.context.bpp_id.clone().unwrap_or_default(),
                bpp_uri: payload.context.bpp_uri.clone().unwrap_or_default(),
            });
        }
    }

    profiles
}

pub async fn handle_on_search(
    app_state: &AppState,
    payload: &WebhookPayload,
    txn_id: &str,
) -> Json<AckResponse> {
    let profiles = extract_profiles_from_on_search(payload, txn_id);

    if let Err(e) = store_profiles(&app_state.db_pool, &profiles).await {
        error!("store_profiles failed: {}", e);
    }

    let pagination = payload
        .message
        .get("pagination")
        .and_then(|p| p.as_object());

    let page = pagination
        .and_then(|p| p.get("page"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1);

    let limit = pagination
        .and_then(|p| p.get("limit"))
        .and_then(|v| v.as_u64())
        .unwrap_or(50);

    let total = pagination
        .and_then(|p| p.get("total"))
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse::<u64>().ok()))
        .unwrap_or(0);

    if total == 0 {
        return ack();
    }

    let total_pages = (total + limit - 1) / limit;

    let bpp_id = payload.context.bpp_id.clone().unwrap_or_default();
    let mut redis = match app_state.redis_pool.get().await {
        Ok(c) => c,
        Err(e) => {
            error!("Redis connection failed: {}", e);
            return ack();
        }
    };

    let base_key = format!("pagination:{}:{}:{}", txn_id, bpp_id, limit);

    let received_key = format!("{}:received", base_key);

    let _: () = redis
        .hset_nx(&base_key, "total_pages", total_pages)
        .await
        .unwrap();

    let _: () = redis.sadd(&received_key, page).await.unwrap();

    let _: () = redis.expire(&base_key, 1800).await.unwrap();
    let _: () = redis.expire(&received_key, 1800).await.unwrap();

    let received_pages: Vec<u64> = redis.smembers(&received_key).await.unwrap();
    let next_page = received_pages.iter().max().copied().unwrap_or(0) + 1;

    if next_page <= total_pages {
        let trigger_key = format!("{}:triggered:{}", base_key, next_page);

        let triggered: bool = redis.set_nx(&trigger_key, 1).await.unwrap();
        let _: () = redis.expire(&trigger_key, 1800).await.unwrap();

        if triggered {
            info!(
                "Triggering next search page: {}/{} (txn_id={}, bpp_id={})",
                next_page, total_pages, txn_id, bpp_id
            );

            let message = SearchMessage {
                intent: Intent {
                    item: None,
                    provider: None,
                    fulfillment: None,
                },
                pagination: Some(Pagination {
                    page: Some(next_page as u32),
                    limit: Some(limit as u32),
                }),
            };

            let request_payload = build_profile_beckn_request(
                &app_state.config,
                txn_id,
                &format!("msg-profile-{}", Uuid::new_v4()),
                &message,
                "search",
                None,
                None,
            );

            let adapter_url = format!("{}/search", app_state.config.bap.caller_uri);

            if let Err(e) = post_json(&adapter_url, request_payload).await {
                error!("Failed to trigger next page {}: {}", next_page, e);
            }
        }
    }

    if received_pages.len() as u64 == total_pages {
        match delete_stale_profiles(&app_state.db_pool, &bpp_id, txn_id).await {
            Ok(count) => info!(
                "🧹 Stale profiles cleaned up: {} rows deleted (bpp_id={}, txn_id={})",
                count, bpp_id, txn_id
            ),
            Err(e) => error!("Stale cleanup failed: {}", e),
        };
    }

    ack()
}

fn ack() -> Json<AckResponse> {
    Json(AckResponse {
        message: AckStatus {
            ack: Ack { status: "ACK" },
        },
    })
}

pub async fn handle_search(
    State(app_state): State<Arc<AppState>>,
    Json(req): Json<ProfileSearchRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let pagination = req.pagination.unwrap_or_default();

    match fetch_profiles(&app_state.db_pool, pagination).await {
        Ok(result) => Ok(Json(serde_json::json!({
            "items": result.items,
            "total": result.total,
            "page": result.page,
            "limit": result.limit
        }))),

        Err(err) => {
            tracing::error!("fetch_profiles failed: {:?}", err);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to fetch profiles"
                })),
            ))
        }
    }
}

pub async fn handle_talent_search(
    State(app_state): State<Arc<AppState>>,
    Json(req): Json<ModelTalentSearchRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (trade, location, experience, radius) = parse_query(req.query.as_deref());

    let parsed_trade = trade.or(req.trade);
    let parsed_location = location.or(req.location.clone());
    let parsed_experience = experience.or(req.experience.clone());
    let parsed_radius = radius.or(req.radius);

    let params = TalentSearchParams {
        trade: parsed_trade,
        location: parsed_location,
        radius: parsed_radius,
        experience: parsed_experience,
        page: req.page.unwrap_or(1).max(1),
        limit: req.limit.unwrap_or(10).clamp(1, 100),
    };

    info!(
        "Searching talent: query={:?}, trade={:?}, location={:?}, experience={:?}, page={}, limit={}",
        req.query, params.trade, params.location, params.experience, params.page, params.limit
    );

    match crate::db::profile::search_talent(&app_state.db_pool, params).await {
        Ok(result) => Ok(Json(serde_json::json!({
            "candidate_count": result.candidate_count,
            "matched_count": result.matched_count,
            "results": result.results,
            "page": result.page,
            "limit": result.limit
        }))),

        Err(err) => {
            tracing::error!("search_talent failed: {:?}", err);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to search talent"
                })),
            ))
        }
    }
}

fn parse_query(
    query: Option<&str>,
) -> (Option<String>, Option<String>, Option<String>, Option<i32>) {
    let query = match query {
        Some(q) => q.to_lowercase(),
        None => return (None, None, None, None),
    };

    let mut trade = None;
    let mut location = None;
    let mut experience = None;
    let mut radius = None;

    let role_keywords = vec![
        "electrician",
        "driver",
        "fitter",
        "mechanic",
        "plumber",
        "carpenter",
        "welder",
        "painter",
        "cook",
        "waiter",
        "security guard",
        "delivery boy",
        "delivery",
        "software engineer",
        "developer",
        "accountant",
        "cashier",
        "sales",
        "marketing",
        "manager",
        "teacher",
        "nurse",
        "doctor",
    ];

    for role in role_keywords {
        if query.contains(role) {
            trade = Some(role.to_string());
            break;
        }
    }

    let location_keywords = vec![
        "bangalore",
        "bengaluru",
        "hyderabad",
        "mumbai",
        "chennai",
        "delhi",
        "pune",
        "hubballi",
        "hubli",
        "mysore",
        "coimbatore",
        "kolkata",
        "ahmedabad",
        "jaipur",
    ];
    for loc in location_keywords {
        if query.contains(loc) {
            location = Some(loc.to_string());
            break;
        }
    }

    if let Some(radius_match) = query
        .split("km")
        .next()
        .or_else(|| query.split("kilometer").next())
    {
        if let Some(num) = radius_match
            .split_whitespace()
            .rev()
            .find(|s| s.chars().all(|c| c.is_ascii_digit()))
        {
            if let Ok(r) = num.parse::<i32>() {
                radius = Some(r);
            }
        }
    }

    if query.contains("fresher") || query.contains("fresher") || query.contains("new") {
        experience = Some("Fresher".to_string());
    } else if let Some(exp_range) = query.matches(char::is_numeric).next() {
        if query.contains("year") {
            experience = Some(format!("{} years", exp_range));
        }
    }

    (trade, location, experience, radius)
}

pub async fn handle_market_insights(
    State(app_state): State<Arc<AppState>>,
    Json(req): Json<ModelMarketInsightsRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let params = MarketInsightsParams {
        role: req.role.clone(),
        location: req.location.clone(),
    };

    info!(
        "Getting market insights: role={:?}, location={:?}",
        params.role, params.location
    );

    match crate::db::profile::get_market_insights(&app_state.db_pool, params).await {
        Ok(result) => {
            let location_dist: Vec<LocationDistribution> = result
                .location_distribution
                .into_iter()
                .map(|(city, count)| LocationDistribution { city, count })
                .collect();

            Ok(Json(serde_json::json!({
                "role": req.role,
                "location": req.location,
                "total_candidates": result.total_candidates,
                "matched_candidates": result.matched_candidates,
                "supply_density": result.supply_density,
                "salary_range": Value::Null,
                "insights": {
                    "experience": {
                        "fresher": result.experience_fresher,
                        "experienced": result.experience_experienced,
                    },
                    "qualification": {
                        "school": result.qualification_school,
                        "college": result.qualification_college,
                        "iti": result.qualification_iti,
                        "certification": result.qualification_certification,
                        "other": result.qualification_other,
                    },
                    "job_type_preference": {
                        "full_time": result.job_full_time,
                        "internship": result.job_internship,
                        "apprenticeship": result.job_apprenticeship,
                        "flexible": result.job_flexible,
                    },
                    "gender_distribution": {
                        "male": result.gender_male,
                        "female": result.gender_female,
                        "other": result.gender_other,
                    },
                    "location_distribution": location_dist,
                }
            })))
        }

        Err(err) => {
            tracing::error!("get_market_insights failed: {:?}", err);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to get market insights"
                })),
            ))
        }
    }
}

pub async fn handle_candidate_details(
    State(app_state): State<Arc<AppState>>,
    Path(profile_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    info!("Getting candidate details for profile_id: {}", profile_id);

    match get_candidate_by_id(&app_state.db_pool, &profile_id).await {
        Ok(Some(candidate)) => Ok(Json(serde_json::json!({
            "profile_id": candidate.profile_id,
            "profile": candidate.profile,
        }))),

        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Candidate not found"
            })),
        )),

        Err(err) => {
            tracing::error!("get_candidate_by_id failed: {:?}", err);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to get candidate details"
                })),
            ))
        }
    }
}
