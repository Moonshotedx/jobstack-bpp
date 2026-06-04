use crate::config::AppConfig;
use crate::utils::http_client::post_json;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

pub async fn send_to_bpp_caller(
    action: &str,
    payload: Value,
    config: Arc<AppConfig>,
) -> Result<Value> {
    let txn_id = payload
        .get("context")
        .and_then(|ctx| ctx.get("transaction_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown_txn");
    let full_action = format!("on{}", action);
    let bap_id = payload
        .get("context")
        .and_then(|ctx| ctx.get("bap_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown_bap_id");
     let bap_uri = payload
        .get("context")
        .and_then(|ctx| ctx.get("bap_uri"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown_bap_uri");

    let bpp_id = payload
        .get("context")
        .and_then(|ctx| ctx.get("bpp_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown_bap_id");
     let bpp_uri = payload
        .get("context")
        .and_then(|ctx| ctx.get("bpp_uri"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown_bap_uri");

    info!(
        target: "bpp",
         "🟡 [BPP → Adapter] Sending request | action: {}, txn_id: {} , bap_id: {}, bap_uri: {} , bpp_id: {}, bpp_uri: {}",
        full_action,
        txn_id,
        bap_id,
        bap_uri,
        bpp_id,
        bpp_uri
    );
    info!(target: "bpp", "──────────────────────────────────────────────");

    let bpp_url = &config.bpp.caller_uri;
    let full_url = format!("{}/{}", bpp_url.trim_end_matches('/'), full_action);
    post_json(&full_url, payload).await
}

pub async fn call_provider_db(path: &str, payload: Value, config: &AppConfig) -> Result<Value> {
    let db_url = &config.provider_db.db_uri;
    let full_url = format!(
        "{}/{}",
        db_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    post_json(&full_url, payload).await
}
