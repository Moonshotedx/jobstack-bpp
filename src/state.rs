use crate::config::AppConfig;
use deadpool_redis::Pool;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub redis_pool: Pool,
    pub db_pool: PgPool,
}
