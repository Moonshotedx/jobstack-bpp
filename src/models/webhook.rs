use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub timestamp: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Context {
    pub domain: String,
    pub action: String,
    pub version: String,
    pub bap_id: String,
    pub bap_uri: String,
    pub transaction_id: String,
    pub message_id: String,
    pub timestamp: String,
    pub bpp_id: Option<String>,
    pub bpp_uri: Option<String>,
    pub ttl: String,
}

#[derive(Debug, Deserialize)]
pub struct WebhookPayload {
    pub context: Context,
    pub message: Value,
}

#[derive(Debug, Serialize)]
pub struct AckResponse {
    pub message: AckStatus,
}

#[derive(Debug, Serialize)]
pub struct AckStatus {
    pub ack: Ack,
}

#[derive(Debug, Serialize)]
pub struct Ack {
    pub status: &'static str,
}
