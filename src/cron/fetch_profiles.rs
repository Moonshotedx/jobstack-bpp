use crate::models::search::{Intent, Pagination, SearchMessage};
use crate::state::AppState;
use crate::utils::http_client::post_json;
use crate::utils::logging::log_cron_job;
use crate::utils::payload_generator::build_profile_beckn_request;
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

pub async fn run(app_state: Arc<AppState>) {
    log_cron_job("🔄", "Starting fetch profiles cron. ");

    let message_id = format!("msg-profile-{}", Uuid::new_v4());
    let txn_id = format!("cron-profile-{}", Uuid::new_v4());
    let intent = Intent {
        item: None,
        provider: None,
        fulfillment: None,
    };

    let message = SearchMessage {
        intent,
        pagination: Some(Pagination {
            page: Some(1),
            limit: Some(50),
        }),
    };

    let payload = build_profile_beckn_request(
        &app_state.config,
        &txn_id,
        &message_id,
        &message,
        "search",
        None,
        None,
    );
    // Send to BAP adapter (profile)
    info!(target: "cron", "📡 Sending search request to BAP adapter...");
    info!(target: "cron", "Payload: {}", payload);
    let adapter_url = format!("{}/search", app_state.config.bap.caller_uri);
    if let Err(e) = post_json(&adapter_url, payload).await {
        error!(target: "cron", "❌ Failed to send search to BAP adapter: {}", e);
    } else {
        info!(target: "cron", "📨 Search request sent to BAP adapter successfully");
    }
}
