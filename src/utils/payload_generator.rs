use crate::config::AppConfig;
use crate::models::webhook::Context;
use chrono::Utc;
use serde::Serialize;
use serde_json::{json, Value};
use tracing::info;

fn generate_context(config: &AppConfig, context: Context) -> Value {
    let now = Utc::now().to_rfc3339();
    let action = format!("on_{}", context.action);
    json!({
        "action": action,
       "bap_id": context.bap_id,
        "bap_uri": context.bap_uri,
        "bpp_id": config.bpp.id,
        "bpp_uri": config.bpp.bpp_uri,
        "domain": config.bpp.domain,
        "message_id": context.message_id,
        "transaction_id": context.transaction_id,
        "timestamp": now,
        "ttl": config.bpp.ttl,
        "version": config.bpp.version
    })
}

pub fn build_beckn_payload(config: &AppConfig, context: Context, db_response: &Value) -> Value {
    let context = generate_context(config, context);

    let mut inner_message = db_response
        .get("message")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(pagination) = db_response.get("pagination") {
        info!("📊 Pagination details: {}", pagination);
        if let Some(message_obj) = inner_message.as_object_mut() {
            message_obj.insert("pagination".to_string(), pagination.clone());
        }
    }

    json!({
        "context": context,
        "message": inner_message
    })
}

fn build_profile_beckn_request_context(
    config: &AppConfig,
    txn_id: &str,
    message_id: &str,
    action: &str,
    bpp_id: Option<&str>,
    bpp_uri: Option<&str>,
) -> Context {
    Context {
        domain: config.bap.domain.clone(),
        action: action.to_string(),
        version: config.bap.version.clone(),
        bap_id: config.bap.id.clone(),
        bap_uri: config.bap.bap_uri.clone(),
        transaction_id: txn_id.to_string(),
        message_id: message_id.to_string(),
        timestamp: Utc::now().to_rfc3339(),
        ttl: config.bap.ttl.clone(),
        bpp_id: bpp_id.map(|v| v.to_string()),
        bpp_uri: bpp_uri.map(|v| v.to_string()),
    }
}

pub fn build_profile_beckn_request(
    config: &AppConfig,
    txn_id: &str,
    message_id: &str,
    message: &impl Serialize,
    action: &str,
    bpp_id: Option<&str>,
    bpp_uri: Option<&str>,
) -> Value {
    let context =
        build_profile_beckn_request_context(config, txn_id, message_id, action, bpp_id, bpp_uri);

    json!({
        "context": context,
        "message": message
    })
}
