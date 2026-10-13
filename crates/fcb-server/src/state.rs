use std::sync::Arc;
use fcb_config::{ConfigManager, RateLimiter};
use fcb_db::DbPool;
use fcb_storage::FileStorage;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub storage: FileStorage,
    pub config: Arc<ConfigManager>,
    #[allow(dead_code)]
    pub rate_limiter: Arc<RateLimiter>,
}
