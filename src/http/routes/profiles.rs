use crate::services::profile::{
    handle_candidate_details, handle_market_insights, handle_search, handle_talent_search,
};
use crate::state::AppState;
use axum::{routing::get, routing::post, Router};
use std::sync::Arc;

pub fn routes(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/search", post(handle_search))
        .route("/v1/talent/search", post(handle_talent_search))
        .route("/v1/talent/insights", post(handle_market_insights))
        .route(
            "/v1/talent/details/{profile_id}",
            get(handle_candidate_details),
        )
        .with_state(app_state)
}
