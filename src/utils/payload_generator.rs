use crate::config::AppConfig;
use crate::models::webhook::Context;
use chrono::Utc;
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
        //"bpp_uri": config.bpp.caller_uri,
        "bpp_uri": config.bpp.bpp_uri,
        "domain": config.bpp.domain,
        "message_id": context.message_id,
        "transaction_id": context.transaction_id,
        "timestamp": now,
        "ttl": "PT30S",
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
