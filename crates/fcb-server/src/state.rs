use std::sync::Arc;
use fcb_config::{ConfigManager, RateLimiter};
use fcb_storage::FileStorage;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub storage: FileStorage,
    pub config: Arc<ConfigManager>,
    #[allow(dead_code)]
    pub rate_limiter: Arc<RateLimiter>,
}
