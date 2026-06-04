use serde::{Deserialize, Serialize};

use crate::models::core::{Location, Tag};

#[derive(Debug, Serialize, Deserialize)]
pub struct Skill {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Person {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<Skill>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Customer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub person: Option<Person>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fulfillment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<Customer>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemDescriptor {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Item {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<ItemDescriptor>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderDescriptor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Provider {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<ProviderDescriptor>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<Location>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Intent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<Item>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment: Option<Fulfillment>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchMessage {
    pub intent: Intent,
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub message: SearchMessage,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequestV2 {
    pub provider: Option<String>,
    pub role: Option<String>,
    pub query: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub primary_filters: Option<String>,
    pub profile: Option<serde_json::Value>,
    pub exclude: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Pagination {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Options {
    pub breif: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TalentSearchRequest {
    pub query: Option<String>,
    pub trade: Option<String>,
    pub location: Option<String>,
    pub radius: Option<i32>,
    pub pay_range_min: Option<i32>,
    pub pay_range_max: Option<i32>,
    pub experience: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MarketInsightsRequest {
    pub role: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExperienceInsights {
    pub fresher: i64,
    pub experienced: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QualificationInsights {
    pub school: i64,
    pub college: i64,
    pub iti: i64,
    pub certification: i64,
    pub other: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobTypePreference {
    pub internship: i64,
    pub apprenticeship: i64,
    pub full_time: i64,
    pub flexible: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenderDistribution {
    pub male: i64,
    pub female: i64,
    pub other: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocationDistribution {
    pub city: String,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarketInsights {
    pub experience: ExperienceInsights,
    pub qualification: QualificationInsights,
    pub job_type_preference: JobTypePreference,
    pub gender_distribution: GenderDistribution,
    pub location_distribution: Vec<LocationDistribution>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarketInsightsResponse {
    pub role: Option<String>,
    pub location: Option<String>,
    pub total_candidates: i64,
    pub matched_candidates: i64,
    pub supply_density: String,
    pub salary_range: Option<String>,
    pub insights: MarketInsights,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TalentSearchResponse {
    pub candidate_count: i64,
    pub sample_candidates: Vec<SampleCandidate>,
    pub page: u32,
    pub limit: u32,
}
