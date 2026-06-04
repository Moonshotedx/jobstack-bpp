use crate::models::search::Pagination;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{query, query_scalar, Error, PgPool, Row};
use tracing::info;

pub struct NewProfile {
    pub profile_id: String,
    pub beckn_structure: Option<Value>,
    pub metadata: Option<Value>,
    pub hash: String,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub transaction_id: String,
    pub bpp_id: String,
    pub bpp_uri: String,
}

#[derive(Debug, Serialize)]
pub struct PaginatedItems<T = serde_json::Value> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: u32,
    pub limit: u32,
}

pub async fn store_profiles(db_pool: &PgPool, profiles: &[NewProfile]) -> Result<(), Error> {
    if profiles.is_empty() {
        return Ok(());
    }
    let profile_ids: Vec<&str> = profiles.iter().map(|p| p.profile_id.as_str()).collect();

    let beckn_structures: Vec<Option<&Value>> = profiles
        .iter()
        .map(|p| p.beckn_structure.as_ref())
        .collect();

    let metadata: Vec<Option<&Value>> = profiles.iter().map(|p| p.metadata.as_ref()).collect();

    let hashes: Vec<&str> = profiles.iter().map(|p| p.hash.as_str()).collect();

    let last_synced_at: Vec<Option<DateTime<Utc>>> =
        profiles.iter().map(|p| p.last_synced_at).collect();

    let transaction_ids: Vec<&str> = profiles.iter().map(|p| p.transaction_id.as_str()).collect();

    let bpp_ids: Vec<&str> = profiles.iter().map(|p| p.bpp_id.as_str()).collect();

    let bpp_uris: Vec<&str> = profiles.iter().map(|p| p.bpp_uri.as_str()).collect();

    query(
        r#"
        INSERT INTO profiles (
            profile_id,
            beckn_structure,
            metadata,
            hash,
            last_synced_at,
            transaction_id,
            bpp_id,
            bpp_uri
        )
        SELECT
            profile_id,
            beckn_structure,
            metadata,
            hash,
            last_synced_at,
            transaction_id,
            bpp_id,
            bpp_uri
        FROM UNNEST(
            $1::text[],
            $2::jsonb[],
            $3::jsonb[],
            $4::text[],
            $5::timestamptz[],
            $6::text[],
            $7::text[],
            $8::text[]
        ) AS t(
            profile_id,
            beckn_structure,
            metadata,
            hash,
            last_synced_at,
            transaction_id,
            bpp_id,
            bpp_uri
        )
        ON CONFLICT (profile_id) DO UPDATE
        SET
            beckn_structure = CASE
                WHEN profiles.hash IS DISTINCT FROM EXCLUDED.hash
                THEN EXCLUDED.beckn_structure
                ELSE profiles.beckn_structure
            END,
            hash = CASE
                WHEN profiles.hash IS DISTINCT FROM EXCLUDED.hash
                THEN EXCLUDED.hash
                ELSE profiles.hash
            END,
            transaction_id = EXCLUDED.transaction_id,
            bpp_id = EXCLUDED.bpp_id,
            bpp_uri = EXCLUDED.bpp_uri,
            last_synced_at = EXCLUDED.last_synced_at
        "#,
    )
    .bind(&profile_ids)
    .bind(&beckn_structures)
    .bind(&metadata)
    .bind(&hashes)
    .bind(&last_synced_at)
    .bind(&transaction_ids)
    .bind(&bpp_ids)
    .bind(&bpp_uris)
    .execute(db_pool)
    .await?;

    Ok(())
}

pub async fn fetch_profiles(
    db_pool: &PgPool,
    pagination: Pagination,
) -> Result<PaginatedItems, sqlx::Error> {
    let page = pagination.page.unwrap_or(1).max(1);
    let limit = pagination.limit.unwrap_or(20).clamp(1, 1000);
    let offset = (page - 1) * limit;

    info!(
        "Fetching profiles (page-limit) - Page: {}, Limit: {}, Offset: {}",
        page, limit, offset
    );

    let total: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
        "#,
    )
    .fetch_one(db_pool)
    .await?;

    let rows = query(
        r#"
        SELECT
            id,
            profile_id,
            beckn_structure,
            transaction_id,
            bpp_id,
            bpp_uri AS bpp_url,
            updated_at
        FROM profiles
        WHERE beckn_structure IS NOT NULL
        ORDER BY updated_at DESC, profile_id DESC
        LIMIT $1
        OFFSET $2
        "#,
    )
    .bind(limit as i64)
    .bind(offset as i64)
    .fetch_all(db_pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.try_get::<i32, _>("id").ok(),
                "profile_id": r.try_get::<String, _>("profile_id").ok(),
                "beckn_structure": r.try_get::<Option<Value>, _>("beckn_structure").ok().flatten(),
                "transaction_id": r.try_get::<String, _>("transaction_id").ok(),
                "bpp_id": r.try_get::<String, _>("bpp_id").ok(),
                "bpp_url": r.try_get::<String, _>("bpp_url").ok(),
                "updated_at": r.try_get::<chrono::DateTime<chrono::Utc>, _>("updated_at").ok()
            })
        })
        .collect::<Vec<_>>();

    Ok(PaginatedItems {
        items,
        total,
        page: page as u32,
        limit: limit as u32,
    })
}

pub async fn delete_stale_profiles(
    db_pool: &PgPool,
    bpp_id: &str,
    txn_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = query(
        r#"
        DELETE FROM profiles
        WHERE bpp_id = $1
          AND transaction_id <> $2
        "#,
    )
    .bind(bpp_id)
    .bind(txn_id)
    .execute(db_pool)
    .await?;

    Ok(result.rows_affected())
}

pub struct TalentSearchParams {
    pub trade: Option<String>,
    pub location: Option<String>,
    pub radius: Option<i32>,
    pub experience: Option<String>,
    pub page: u32,
    pub limit: u32,
}

pub struct TalentSearchResult {
    pub candidate_count: i64,
    pub matched_count: i64,
    pub results: Vec<SampleCandidate>,
    pub page: u32,
    pub limit: u32,
}

#[derive(Debug, Serialize)]
pub struct SampleCandidate {
    pub profile_id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub location: Option<String>,
    pub work_experience: Option<String>,
    pub work_experience_years: Option<String>,
    pub last_role_held: Option<String>,
    pub qualification: Option<String>,
    pub job_roles_interested_in: Option<String>,
    pub jobs_interested_in: Option<Vec<String>>,
}

pub async fn search_talent(
    db_pool: &PgPool,
    params: TalentSearchParams,
) -> Result<TalentSearchResult, sqlx::Error> {
    let page = params.page.max(1);
    let limit = params.limit.clamp(1, 100);
    let offset = (page - 1) * limit;

    let trade_pattern = params.trade.as_ref().map(|t| format!("%{}%", t));
    let location_pattern = params.location.as_ref().map(|l| format!("%{}%", l));
    let experience_pattern = params.experience.as_ref().map(|e| format!("%{}%", e));

    let candidate_count: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
        "#,
    )
    .fetch_one(db_pool)
    .await?;

    let matched_count: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND ($3::text IS NULL OR 
               beckn_structure->'tags'->'profile'->'whatIHave'->>'workExperience' ILIKE $3)
        "#,
    )
    .bind(&trade_pattern)
    .bind(&location_pattern)
    .bind(&experience_pattern)
    .fetch_one(db_pool)
    .await?;

    let rows = query(
        r#"
        SELECT
            id,
            profile_id,
            beckn_structure,
            updated_at
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND ($3::text IS NULL OR 
               beckn_structure->'tags'->'profile'->'whatIHave'->>'workExperience' ILIKE $3)
        ORDER BY updated_at DESC, profile_id DESC
        LIMIT $4
        OFFSET $5
        "#,
    )
    .bind(&trade_pattern)
    .bind(&location_pattern)
    .bind(&experience_pattern)
    .bind(limit as i64)
    .bind(offset as i64)
    .fetch_all(db_pool)
    .await?;

    let results = rows
        .into_iter()
        .map(|r| {
            let beckn: Option<Value> = r.try_get("beckn_structure").ok();
            let profile = beckn
                .as_ref()
                .and_then(|b| b.get("tags"))
                .and_then(|t| t.get("profile"));

            let who_i_am = profile.and_then(|p| p.get("whoIAm"));
            let what_i_have = profile.and_then(|p| p.get("whatIHave"));
            let what_i_want = profile.and_then(|p| p.get("whatIWant"));

            let name = who_i_am
                .and_then(|w| w.get("name"))
                .and_then(|n| n.as_str())
                .map(String::from);

            let role = profile
                .and_then(|p| p.get("role"))
                .and_then(|r| r.as_str())
                .map(String::from);

            let location = profile
                .and_then(|p| p.get("location"))
                .and_then(|l| l.get("address"))
                .or_else(|| {
                    profile
                        .and_then(|p| p.get("location"))
                        .and_then(|l| l.get("city"))
                })
                .or_else(|| {
                    profile
                        .and_then(|p| p.get("locationdata"))
                        .and_then(|l| l.get("city"))
                })
                .or_else(|| {
                    profile
                        .and_then(|p| p.get("locationdata"))
                        .and_then(|l| l.get("address"))
                })
                .and_then(|v| v.as_str())
                .map(String::from);

            let work_experience = what_i_have
                .and_then(|w| w.get("workExperience"))
                .and_then(|w| w.as_str())
                .map(String::from);

            let work_experience_years = what_i_have
                .and_then(|w| w.get("workExperienceYears"))
                .and_then(|w| w.as_str())
                .map(String::from);

            let last_role_held = what_i_have
                .and_then(|w| w.get("nameOfLastRoleHeld"))
                .and_then(|n| n.as_str())
                .map(String::from);

            let qualification = what_i_have
                .and_then(|w| w.get("highestQualificationOrSkill"))
                .and_then(|h| h.get("category"))
                .and_then(|c| c.as_str())
                .map(String::from);

            let job_roles_interested_in = what_i_want
                .and_then(|w| w.get("nameOfJobRolesInterestedIn"))
                .and_then(|n| n.as_str())
                .map(String::from);

            let jobs_interested_in = what_i_want
                .and_then(|w| w.get("natureOfJobsInterestedIn"))
                .and_then(|n| n.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                });

            SampleCandidate {
                profile_id: r.try_get::<String, _>("profile_id").unwrap_or_default(),
                name,
                role,
                location,
                work_experience,
                work_experience_years,
                last_role_held,
                qualification,
                job_roles_interested_in,
                jobs_interested_in,
            }
        })
        .collect();

    Ok(TalentSearchResult {
        candidate_count,
        matched_count,
        results,
        page,
        limit,
    })
}

pub struct MarketInsightsParams {
    pub role: Option<String>,
    pub location: Option<String>,
}

pub struct MarketInsightsResult {
    pub total_candidates: i64,
    pub matched_candidates: i64,
    pub supply_density: String,
    pub experience_fresher: i64,
    pub experience_experienced: i64,
    pub qualification_school: i64,
    pub qualification_college: i64,
    pub qualification_iti: i64,
    pub qualification_certification: i64,
    pub qualification_other: i64,
    pub job_internship: i64,
    pub job_apprenticeship: i64,
    pub job_full_time: i64,
    pub job_flexible: i64,
    pub gender_male: i64,
    pub gender_female: i64,
    pub gender_other: i64,
    pub location_distribution: Vec<(String, i64)>,
}

pub async fn get_market_insights(
    db_pool: &PgPool,
    params: MarketInsightsParams,
) -> Result<MarketInsightsResult, sqlx::Error> {
    let role_pattern = params.role.as_ref().map(|r| format!("%{}%", r));
    let location_pattern = params.location.as_ref().map(|l| format!("%{}%", l));

    let total_candidates: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
        "#,
    )
    .fetch_one(db_pool)
    .await?;

    let matched_candidates: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
        "#,
    )
    .bind(&role_pattern)
    .bind(&location_pattern)
    .fetch_one(db_pool)
    .await?;

    let supply_density = if matched_candidates == 0 {
        "None".to_string()
    } else if matched_candidates < 50 {
        "Low".to_string()
    } else if matched_candidates < 200 {
        "Medium".to_string()
    } else {
        "High".to_string()
    };

    let experience_fresher: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND beckn_structure->'tags'->'profile'->'whatIHave'->>'workExperience' = 'Fresher'
        "#,
    )
    .bind(&role_pattern)
    .bind(&location_pattern)
    .fetch_one(db_pool)
    .await?;

    let experience_experienced = matched_candidates - experience_fresher;

    let qualification_school: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND beckn_structure->'tags'->'profile'->'whatIHave'->'highestQualificationOrSkill'->>'category' = 'School'
        "#,
    )
    .bind(&role_pattern)
    .bind(&location_pattern)
    .fetch_one(db_pool)
    .await?;

    let qualification_college: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND beckn_structure->'tags'->'profile'->'whatIHave'->'highestQualificationOrSkill'->>'category' = 'College'
        "#,
    )
    .bind(&role_pattern)
    .bind(&location_pattern)
    .fetch_one(db_pool)
    .await?;

    let qualification_iti: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND beckn_structure->'tags'->'profile'->'whatIHave'->'highestQualificationOrSkill'->>'category' ILIKE '%ITI%'
        "#,
    )
    .bind(&role_pattern)
    .bind(&location_pattern)
    .fetch_one(db_pool)
    .await?;

    let qualification_certification: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND beckn_structure->'tags'->'profile'->'whatIHave'->'highestQualificationOrSkill'->>'category' ILIKE '%Certification%'
        "#,
    )
    .bind(&role_pattern)
    .bind(&location_pattern)
    .fetch_one(db_pool)
    .await?;

    let qualification_other = matched_candidates
        - qualification_school
        - qualification_college
        - qualification_iti
        - qualification_certification;

    let job_internship: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND beckn_structure->'tags'->'profile'->'whatIWant'->'natureOfJobsInterestedIn' ? 'Internship'
        "#,
    )
    .bind(&role_pattern)
    .bind(&location_pattern)
    .fetch_one(db_pool)
    .await?;

    let job_apprenticeship: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND beckn_structure->'tags'->'profile'->'whatIWant'->'natureOfJobsInterestedIn' ? 'Apprenticeship'
        "#,
    )
    .bind(&role_pattern)
    .bind(&location_pattern)
    .fetch_one(db_pool)
    .await?;

    let job_full_time: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND beckn_structure->'tags'->'profile'->'whatIWant'->'natureOfJobsInterestedIn' ? 'Full-time'
        "#,
    )
    .bind(&role_pattern)
    .bind(&location_pattern)
    .fetch_one(db_pool)
    .await?;

    let job_flexible: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND beckn_structure->'tags'->'profile'->'whatIWant'->'natureOfJobsInterestedIn' ? 'Flexible'
        "#,
    )
    .bind(&role_pattern)
    .bind(&location_pattern)
    .fetch_one(db_pool)
    .await?;

    let gender_male: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND beckn_structure->'tags'->'profile'->'whoIAm'->>'gender' = 'Male'
        "#,
    )
    .bind(&role_pattern)
    .bind(&location_pattern)
    .fetch_one(db_pool)
    .await?;

    let gender_female: i64 = query_scalar(
        r#"
        SELECT COUNT(*) 
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
          AND ($2::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'location' ILIKE $2
               OR beckn_structure->'tags'->'profile'->>'city' ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'locationdata'->>'city') ILIKE $2
               OR (beckn_structure->'tags'->'profile'->'location'->>'city') ILIKE $2)
          AND beckn_structure->'tags'->'profile'->'whoIAm'->>'gender' = 'Female'
        "#,
    )
    .bind(&role_pattern)
    .bind(&location_pattern)
    .fetch_one(db_pool)
    .await?;

    let gender_other = matched_candidates - gender_male - gender_female;

    let location_rows = query(
        r#"
        SELECT 
            COALESCE(
                beckn_structure->'tags'->'profile'->'location'->>'city',
                beckn_structure->'tags'->'profile'->'locationdata'->>'city',
                beckn_structure->'tags'->'profile'->>'city',
                'Unknown'
            ) as city,
            COUNT(*) as count
        FROM profiles
        WHERE beckn_structure IS NOT NULL
          AND ($1::text IS NULL OR 
               beckn_structure->'tags'->'profile'->>'role' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIWant'->>'nameOfJobRolesInterestedIn' ILIKE $1
               OR beckn_structure->'tags'->'profile'->'whatIHave'->>'nameOfLastRoleHeld' ILIKE $1)
        GROUP BY city
        ORDER BY count DESC
        LIMIT 10
        "#,
    )
    .bind(&role_pattern)
    .fetch_all(db_pool)
    .await?;

    let location_distribution: Vec<(String, i64)> = location_rows
        .into_iter()
        .map(|r| {
            let city: String = r.try_get("city").unwrap_or_else(|_| "Unknown".to_string());
            let count: i64 = r.try_get("count").unwrap_or(0);
            (city, count)
        })
        .collect();

    Ok(MarketInsightsResult {
        total_candidates,
        matched_candidates,
        supply_density,
        experience_fresher,
        experience_experienced,
        qualification_school,
        qualification_college,
        qualification_iti,
        qualification_certification,
        qualification_other,
        job_internship,
        job_apprenticeship,
        job_full_time,
        job_flexible,
        gender_male,
        gender_female,
        gender_other,
        location_distribution,
    })
}

#[derive(Debug, Serialize)]
pub struct CandidateDetails {
    pub profile_id: String,
    pub profile: Value,
}

pub async fn get_candidate_by_id(
    db_pool: &PgPool,
    profile_id: &str,
) -> Result<Option<CandidateDetails>, sqlx::Error> {
    let row = query(
        r#"
        SELECT profile_id, beckn_structure
        FROM profiles
        WHERE profile_id = $1
        "#,
    )
    .bind(profile_id)
    .fetch_optional(db_pool)
    .await?;

    match row {
        Some(r) => {
            let beckn_structure: Option<Value> = r.try_get("beckn_structure").ok().flatten();
            let profile = beckn_structure
                .as_ref()
                .and_then(|b| b.get("tags"))
                .and_then(|t| t.get("profile"))
                .cloned()
                .unwrap_or(Value::Null);

            Ok(Some(CandidateDetails {
                profile_id: r.try_get::<String, _>("profile_id").unwrap_or_default(),
                profile,
            }))
        }
        None => Ok(None),
    }
}
