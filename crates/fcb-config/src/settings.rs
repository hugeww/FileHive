use std::sync::Arc;
use arc_swap::ArcSwap;
use fcb_core::AppError;
use fcb_db::crud::{get_key_value, set_key_value};
use fcb_db::DbPool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ADMIN_SESSION_EXPIRE_DEFAULT: i64 = 30 * 24 * 60 * 60; // 2592000 (30 days)
pub const ADMIN_SESSION_EXPIRE_MIN: i64 = 24 * 60 * 60;          // 86400 (1 day)
pub const ADMIN_SESSION_EXPIRE_MAX: i64 = 365 * 24 * 60 * 60;    // 31536000 (365 days)

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeChoice {
    pub name: String,
    pub key: String,
    pub author: String,
    pub version: String,
}

/// 系统全局配置，默认值与 Python 版 core/settings.py 中的 DEFAULT_CONFIG 保持严格一致
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SystemSettings {
    pub file_storage: String,
    pub storage_path: String,
    pub storage_limit: i64,
    pub name: String,
    pub description: String,
    pub notify_title: String,
    pub notify_content: String,
    pub page_explain: String,
    pub keywords: String,
    pub s3_access_key_id: String,
    pub s3_secret_access_key: String,
    pub s3_bucket_name: String,
    pub s3_endpoint_url: String,
    pub s3_region_name: String,
    pub s3_signature_version: String,
    pub s3_hostname: String,
    pub s3_addressing_style: String,
    pub s3_proxy: i32,
    pub max_save_seconds: i64,
    pub aws_session_token: String,
    pub onedrive_domain: String,
    pub onedrive_client_id: String,
    pub onedrive_username: String,
    pub onedrive_password: String,
    pub onedrive_root_path: String,
    pub onedrive_proxy: i32,
    pub webdav_root_path: String,
    pub webdav_proxy: i32,
    pub admin_token: String,
    pub jwt_secret: String,
    pub admin_session_expire: i64,
    pub open_upload: i32,
    pub mcp_require_token: i32,
    pub upload_size: i64,
    pub allowed_file_types: Vec<String>,
    pub expire_style: Vec<String>,
    pub code_generate_type: String,
    pub upload_minute: i32,
    pub enable_chunk: i32,
    pub webdav_url: String,
    pub webdav_password: String,
    pub webdav_username: String,
    pub opacity: f64,
    pub background: String,
    pub upload_count: i32,
    pub themes_choices: Vec<ThemeChoice>,
    pub themes_select: String,
    pub error_minute: i32,
    pub error_count: i32,
    pub login_count: i32,
    pub login_minute: i32,
    pub server_workers: i32,
    pub server_host: String,
    pub server_port: u16,
    pub show_admin_addr: i32,
    pub robots_text: String,
    pub trusted_proxies: Vec<String>,
    pub chunk_expire_hours: i64,
    pub opendal_scheme: String,
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self {
            file_storage: "local".to_string(),
            storage_path: "".to_string(),
            storage_limit: 0,
            name: "文件蜂巢 - FileHive".to_string(),
            description: "像取快递一样取文件 · 基于 Rust 的极速轻量文件暂存系统".to_string(),
            notify_title: "系统通知".to_string(),
            notify_content: "欢迎使用 FileHive 文件蜂巢。".to_string(),
            page_explain: "请勿上传或分享违法内容。根据《中华人民共和国网络安全法》、《中华人民共和国刑法》、《中华人民共和国治安管理处罚法》等相关规定。 传播或存储违法、违规内容，会受到相关处罚，严重者将承担刑事责任。本站坚决配合相关部门，确保网络内容的安全，和谐，打造绿色网络环境。".to_string(),
            keywords: "FileHive, 文件蜂巢, 提取码, 口令传送, 匿名分享, 极速文件柜".to_string(),
            s3_access_key_id: "".to_string(),
            s3_secret_access_key: "".to_string(),
            s3_bucket_name: "".to_string(),
            s3_endpoint_url: "".to_string(),
            s3_region_name: "auto".to_string(),
            s3_signature_version: "s3v2".to_string(),
            s3_hostname: "".to_string(),
            s3_addressing_style: "auto".to_string(),
            s3_proxy: 0,
            max_save_seconds: 0,
            aws_session_token: "".to_string(),
            onedrive_domain: "".to_string(),
            onedrive_client_id: "".to_string(),
            onedrive_username: "".to_string(),
            onedrive_password: "".to_string(),
            onedrive_root_path: "filebox_storage".to_string(),
            onedrive_proxy: 0,
            webdav_root_path: "filebox_storage".to_string(),
            webdav_proxy: 0,
            admin_token: "".to_string(),
            jwt_secret: "".to_string(),
            admin_session_expire: ADMIN_SESSION_EXPIRE_DEFAULT,
            open_upload: 1,
            mcp_require_token: 0,
            upload_size: 10 * 1024 * 1024,
            allowed_file_types: vec!["*".to_string()],
            expire_style: vec![
                "day".to_string(),
                "hour".to_string(),
                "minute".to_string(),
                "forever".to_string(),
                "count".to_string(),
            ],
            code_generate_type: "secret".to_string(),
            upload_minute: 1,
            enable_chunk: 0,
            webdav_url: "".to_string(),
            webdav_password: "".to_string(),
            webdav_username: "".to_string(),
            opacity: 0.9,
            background: "".to_string(),
            upload_count: 10,
            themes_choices: vec![
                ThemeChoice {
                    name: "2023".to_string(),
                    key: "themes/2023".to_string(),
                    author: "Lan".to_string(),
                    version: "1.0".to_string(),
                },
                ThemeChoice {
                    name: "2024".to_string(),
                    key: "themes/2024".to_string(),
                    author: "Lan".to_string(),
                    version: "1.0".to_string(),
                },
            ],
            themes_select: "themes/2024".to_string(),
            error_minute: 1,
            error_count: 10,
            login_count: 5,
            login_minute: 15,
            server_workers: 1,
            server_host: "0.0.0.0".to_string(),
            server_port: 12345,
            show_admin_addr: 1,
            robots_text: "User-agent: *\nDisallow: /".to_string(),
            trusted_proxies: vec![],
            chunk_expire_hours: 24,
            opendal_scheme: "s3".to_string(),
        }
    }
}

/// 配置管理器，持有热更新配置与数据库连接池
#[derive(Clone)]
pub struct ConfigManager {
    settings: Arc<ArcSwap<SystemSettings>>,
    pool: DbPool,
}

impl ConfigManager {
    /// 基于数据库连接池创建配置管理器，初始配置为默认值
    pub fn new(pool: DbPool) -> Self {
        Self {
            settings: Arc::new(ArcSwap::from_pointee(SystemSettings::default())),
            pool,
        }
    }

    /// 基于已有的设置结构创建配置管理器
    pub fn with_settings(pool: DbPool, settings: SystemSettings) -> Self {
        Self {
            settings: Arc::new(ArcSwap::from_pointee(settings)),
            pool,
        }
    }

    /// 获取底层 Arc<ArcSwap<SystemSettings>>
    pub fn settings_arc(&self) -> Arc<ArcSwap<SystemSettings>> {
        self.settings.clone()
    }

    /// 获取底层 DbPool
    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    /// 获取当前配置的只读守卫
    pub fn get(&self) -> arc_swap::Guard<Arc<SystemSettings>> {
        self.settings.load()
    }

    /// 从 keyvalue 表中读取 key = 'settings' 并解析合并到运行时
    pub async fn load_from_db(&self) -> Result<(), AppError> {
        let kv_opt = get_key_value(&self.pool, "settings").await?;

        if let Some(kv) = kv_opt {
            if let Some(json_str) = kv.value {
                if !json_str.trim().is_empty() {
                    // 基于当前默认值进行字段合并
                    let mut base_val = serde_json::to_value(SystemSettings::default())
                        .map_err(|e| AppError::Internal(e.to_string()))?;

                    if let Ok(Value::Object(db_map)) = serde_json::from_str::<Value>(&json_str) {
                        if let Value::Object(ref mut base_map) = base_val {
                            for (k, v) in db_map {
                                base_map.insert(k, v);
                            }
                        }
                    }

                    let loaded: SystemSettings = serde_json::from_value(base_val)
                        .map_err(|e| AppError::Internal(format!("Failed to parse merged settings: {}", e)))?;
                    self.settings.store(Arc::new(loaded));
                }
            }
        }
        Ok(())
    }

    /// 更新配置：将新字段合并到现有配置中，并持久化到 keyvalue 表
    pub async fn update(&self, value: Value) -> Result<(), AppError> {
        let update_map = match value {
            Value::Object(map) => map,
            _ => return Err(AppError::BadRequest("Settings update must be a JSON object".to_string())),
        };

        let current = (**self.settings.load()).clone();
        let mut current_val = serde_json::to_value(&current)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if let Value::Object(ref mut curr_map) = current_val {
            for (k, v) in update_map {
                curr_map.insert(k, v);
            }
        }

        let new_settings: SystemSettings = serde_json::from_value(current_val.clone())
            .map_err(|e| AppError::BadRequest(format!("Invalid settings update: {}", e)))?;

        let json_str = serde_json::to_string(&new_settings)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        set_key_value(&self.pool, "settings", &json_str).await?;

        self.settings.store(Arc::new(new_settings));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_settings_defaults() {
        let settings = SystemSettings::default();
        assert_eq!(settings.file_storage, "local");
        assert_eq!(settings.storage_limit, 0);
        assert_eq!(settings.name, "文件蜂巢 - FileHive");
        assert_eq!(settings.upload_count, 10);
        assert_eq!(settings.upload_minute, 1);
        assert_eq!(settings.login_count, 5);
        assert_eq!(settings.login_minute, 15);
        assert_eq!(settings.error_count, 10);
        assert_eq!(settings.error_minute, 1);
        assert_eq!(settings.server_port, 12345);
        assert_eq!(settings.server_host, "0.0.0.0");
        assert_eq!(settings.themes_choices.len(), 2);
        assert_eq!(settings.themes_select, "themes/2024");
        assert_eq!(settings.opendal_scheme, "s3");
    }

    #[test]
    fn test_system_settings_serde_partial() {
        let json = r#"{"name": "自定义快递柜", "upload_count": 25}"#;
        let parsed: SystemSettings = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.name, "自定义快递柜");
        assert_eq!(parsed.upload_count, 25);
        assert_eq!(parsed.file_storage, "local");
        assert_eq!(parsed.server_port, 12345);
    }

    #[tokio::test]
    async fn test_config_manager_db_lifecycle() {
        let pool = fcb_db::init_pool(":memory:").await.unwrap();

        let manager = ConfigManager::new(pool.clone());
        assert_eq!(manager.get().name, "文件蜂巢 - FileHive");

        // 更新配置
        let update_val = serde_json::json!({
            "name": "新快递柜",
            "upload_count": 50
        });
        manager.update(update_val).await.unwrap();

        assert_eq!(manager.get().name, "新快递柜");
        assert_eq!(manager.get().upload_count, 50);

        // 新建 manager 并通过 load_from_db 加载
        let manager2 = ConfigManager::new(pool);
        manager2.load_from_db().await.unwrap();
        assert_eq!(manager2.get().name, "新快递柜");
        assert_eq!(manager2.get().upload_count, 50);
        assert_eq!(manager2.get().server_port, 12345);
    }
}
