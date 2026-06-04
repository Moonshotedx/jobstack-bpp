pub mod profiles;
pub mod webhook;
use crate::middleware::api_key::api_key_auth;
use crate::state::AppState;
use axum::{middleware, response::IntoResponse, routing::get, Json, Router};
use chrono::Utc;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::models::webhook::HealthResponse;
async fn health_check() -> impl IntoResponse {
    let response = HealthResponse {
        status: "OK",
        timestamp: Utc::now().to_rfc3339(),
    };

    Json(response)
}

pub fn create_routes(app_state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        .merge(profiles::routes(app_state.clone()))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            api_key_auth,
        ));

    Router::new()
        .route("/", get(health_check))
        .nest("/api", api_routes)
        .merge(webhook::routes(app_state))
        .layer(cors)
}
