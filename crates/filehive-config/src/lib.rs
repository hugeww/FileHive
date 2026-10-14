pub mod quota;
pub mod rate_limit;
pub mod settings;

pub use quota::{
    get_storage_usage, release_storage, reserve_storage, reserve_storage_with_limit, StorageUsage,
};
pub use rate_limit::RateLimiter;
pub use settings::{
    ConfigManager, SystemSettings, ThemeChoice, ADMIN_SESSION_EXPIRE_DEFAULT,
    ADMIN_SESSION_EXPIRE_MAX, ADMIN_SESSION_EXPIRE_MIN,
};

#[cfg(test)]
mod tests {
    use super::*;
    use filehive_db::init_pool;
    use serde_json::json;

    #[tokio::test]
    async fn test_rate_limiter() {
        let pool = init_pool("sqlite::memory:").await.unwrap();
        let config_mgr = ConfigManager::new(pool.clone());
        let limiter = RateLimiter::new(config_mgr.settings_arc());
        let ip = "192.168.1.100";

        // 默认 upload_count=10, upload_minute=1
        for _ in 0..10 {
            assert!(limiter.check_ip(ip, "upload").is_ok());
        }
        // 第 11 次被限流
        assert!(limiter.check_ip(ip, "upload").is_err());
        limiter.clear();
        assert!(limiter.check_ip(ip, "upload").is_ok());
    }

    #[tokio::test]
    async fn test_config_manager_and_quota() {
        let pool = init_pool("sqlite::memory:").await.unwrap();
        let config_mgr = ConfigManager::new(pool.clone());
        config_mgr.load_from_db().await.unwrap();

        assert_eq!(config_mgr.get().name, "文件蜂巢 - FileHive");

        config_mgr
            .update(json!({
                "name": "Custom FCB",
                "storage_limit": 1024000
            }))
            .await
            .unwrap();

        assert_eq!(config_mgr.get().name, "Custom FCB");
        assert_eq!(config_mgr.get().storage_limit, 1024000);

        reserve_storage(&pool, "token-1", 5000, 60).await.unwrap();
        release_storage(&pool, "token-1").await.unwrap();
    }
}
