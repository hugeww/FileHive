use std::sync::Arc;
use fcb_config::{ConfigManager, RateLimiter, SystemSettings};
use fcb_db::DbPool;
use fcb_storage::{FileStorage, StorageConfig};

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub storage: FileStorage,
    pub config: Arc<ConfigManager>,
    #[allow(dead_code)]
    pub rate_limiter: Arc<RateLimiter>,
}

/// 根据系统配置构建对应的存储配置（支持 Local、S3/MinIO、WebDAV）
pub fn storage_config_from_settings(settings: &SystemSettings) -> StorageConfig {
    match settings.file_storage.to_lowercase().as_str() {
        "s3" | "minio" => StorageConfig::S3 {
            endpoint: settings.s3_endpoint_url.clone(),
            bucket: settings.s3_bucket_name.clone(),
            access_key_id: settings.s3_access_key_id.clone(),
            secret_access_key: settings.s3_secret_access_key.clone(),
            region: if settings.s3_region_name.is_empty() {
                "auto".to_string()
            } else {
                settings.s3_region_name.clone()
            },
            root: None,
        },
        "webdav" => StorageConfig::WebDAV {
            endpoint: settings.webdav_url.clone(),
            username: settings.webdav_username.clone(),
            password: settings.webdav_password.clone(),
            root_path: if settings.webdav_root_path.is_empty() {
                "filebox_storage".to_string()
            } else {
                settings.webdav_root_path.clone()
            },
            token: None,
        },
        _ => StorageConfig::Local {
            storage_path: if settings.storage_path.is_empty() {
                "data/share".to_string()
            } else {
                settings.storage_path.clone()
            },
        },
    }
}
