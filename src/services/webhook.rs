use crate::config::AppConfig;
use crate::models::webhook::Context;
use crate::models::webhook::{Ack, AckResponse, AckStatus, WebhookPayload};
use crate::services::{
    confirm::handle_confirm, init::handle_init, profile::handle_on_search, search::handle_search,
    select::handle_select, status::handle_status,
};
use crate::state::AppState;
use crate::utils::mock_responses::load_mock_response;
use crate::workers::processor::spawn_processing_task;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

pub async fn generate_response(
    action: &str,
    context: Context,
    message: Value,
    config: &AppConfig,
) -> anyhow::Result<Value> {
    if !config.use_mock_bpp_response {
        match action {
            "search" => handle_search(context, message, config).await,
            "select" => handle_select(context, message, config).await,
            "init" => handle_init(context, message, config).await,
            "confirm" => handle_confirm(context, message, config).await,
            "status" => handle_status(context, message, config).await,
            _ => Ok(serde_json::json!({
                "context": context,
                "message": {
                    "error": format!("Unsupported action: {}", action)
                }
            })),
        }
    } else {
        info!("Fallback to mock response: ...");
        let mut mock = load_mock_response(action).unwrap_or_else(|| {
            serde_json::json!({
                "context": context,
                "message": {
                    "note": "Default mock response",
                    "action": action
                }
            })
        });

        if let Some(ctx) = mock.get_mut("context") {
            ctx["transaction_id"] = serde_json::json!(context.transaction_id.clone());
            ctx["message_id"] = serde_json::json!(context.message_id.clone());
        }

        if action == "confirm" {
            if let Some(order) = mock.get_mut("message").and_then(|msg| msg.get_mut("order")) {
                order["id"] = serde_json::json!(Uuid::new_v4().to_string());
            }
        }

        Ok(mock)
    }
}

pub async fn webhook_handler(
    Path(action): Path<String>,
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<WebhookPayload>,
) -> impl IntoResponse {
    info!(
        target: "webhook",
        "🟢 [ Adapter → BPP] Request received | txn_id: {}, msg_id: {}, action: {}, timestamp: {}",
        payload.context.transaction_id,
        payload.context.message_id,
        payload.context.action,
        payload.context.timestamp
    );

    debug!(target: "webhook", "🔎 Message payload: {:?}", payload.message);
    if action.starts_with("on_") {
        info!(
            "Skipping processing since action starts with 'on_': {:?}",
            action
        );
        let ack = AckResponse {
            message: AckStatus {
                ack: Ack { status: "ACK" },
            },
        };
        return Json(ack);
    }

    spawn_processing_task(
        payload.context,
        payload.message,
        action,
        app_state.config.clone(),
    );

    let ack = AckResponse {
        message: AckStatus {
            ack: Ack { status: "ACK" },
        },
    };

    Json(ack)
}

pub async fn webhook_handler_profiles(
    Path(action): Path<String>,
    State(app_state): State<Arc<AppState>>,
    Json(payload): Json<WebhookPayload>,
) -> impl IntoResponse {
    let txn_id = payload.context.transaction_id.clone();

    info!("webhook called: action = {}, txn_id = {}", action, txn_id);

    match action.as_str() {
        "on_search" => handle_on_search(&app_state, &payload, &txn_id).await,
        _ => {
            info!("Unsupported action for profiles: {}", action);
            Json(AckResponse {
                message: AckStatus {
                    ack: Ack { status: "ACK" },
                },
            })
        }
    }
}
