use serde::{Deserialize, Serialize};

/// 存储类型配置枚举，支持 Local、S3、WebDAV 与 OneDrive
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StorageConfig {
    #[serde(alias = "local", alias = "Local")]
    Local {
        #[serde(default = "default_local_path")]
        storage_path: String,
    },
    #[serde(alias = "s3", alias = "S3")]
    S3 {
        #[serde(default)]
        endpoint: String,
        bucket: String,
        access_key_id: String,
        secret_access_key: String,
        #[serde(default = "default_s3_region")]
        region: String,
        #[serde(default)]
        root: Option<String>,
    },
    #[serde(alias = "webdav", alias = "WebDAV", alias = "web_dav", alias = "WebDav")]
    WebDAV {
        endpoint: String,
        #[serde(default)]
        username: String,
        #[serde(default)]
        password: String,
        #[serde(default = "default_webdav_root")]
        root_path: String,
        #[serde(default)]
        token: Option<String>,
    },
    #[serde(alias = "onedrive", alias = "OneDrive", alias = "one_drive")]
    OneDrive {
        #[serde(default)]
        client_id: String,
        #[serde(default)]
        client_secret: String,
        #[serde(default)]
        refresh_token: String,
        #[serde(default)]
        access_token: String,
        #[serde(default = "default_onedrive_root")]
        root_path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalStorageConfig {
    pub storage_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct S3StorageConfig {
    pub endpoint: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
    pub root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebDAVStorageConfig {
    pub endpoint: String,
    pub username: String,
    pub password: String,
    pub root_path: String,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OneDriveStorageConfig {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
    pub access_token: String,
    pub root_path: String,
}

pub fn default_local_path() -> String {
    "data/share".to_string()
}

pub fn default_s3_region() -> String {
    "auto".to_string()
}

pub fn default_webdav_root() -> String {
    "filebox_storage".to_string()
}

pub fn default_onedrive_root() -> String {
    "filebox_storage".to_string()
}

impl Default for StorageConfig {
    fn default() -> Self {
        StorageConfig::Local {
            storage_path: default_local_path(),
        }
    }
}

impl StorageConfig {
    pub fn local(path: impl Into<String>) -> Self {
        Self::Local {
            storage_path: path.into(),
        }
    }

    pub fn s3(
        bucket: impl Into<String>,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
        endpoint: impl Into<String>,
        region: impl Into<String>,
    ) -> Self {
        Self::S3 {
            endpoint: endpoint.into(),
            bucket: bucket.into(),
            access_key_id: access_key_id.into(),
            secret_access_key: secret_access_key.into(),
            region: region.into(),
            root: None,
        }
    }

    pub fn webdav(
        endpoint: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        root_path: impl Into<String>,
    ) -> Self {
        Self::WebDAV {
            endpoint: endpoint.into(),
            username: username.into(),
            password: password.into(),
            root_path: root_path.into(),
            token: None,
        }
    }

    pub fn onedrive(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        refresh_token: impl Into<String>,
        access_token: impl Into<String>,
        root_path: impl Into<String>,
    ) -> Self {
        Self::OneDrive {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            refresh_token: refresh_token.into(),
            access_token: access_token.into(),
            root_path: root_path.into(),
        }
    }
}

impl From<LocalStorageConfig> for StorageConfig {
    fn from(cfg: LocalStorageConfig) -> Self {
        StorageConfig::Local {
            storage_path: cfg.storage_path,
        }
    }
}

impl From<S3StorageConfig> for StorageConfig {
    fn from(cfg: S3StorageConfig) -> Self {
        StorageConfig::S3 {
            endpoint: cfg.endpoint,
            bucket: cfg.bucket,
            access_key_id: cfg.access_key_id,
            secret_access_key: cfg.secret_access_key,
            region: cfg.region,
            root: cfg.root,
        }
    }
}

impl From<WebDAVStorageConfig> for StorageConfig {
    fn from(cfg: WebDAVStorageConfig) -> Self {
        StorageConfig::WebDAV {
            endpoint: cfg.endpoint,
            username: cfg.username,
            password: cfg.password,
            root_path: cfg.root_path,
            token: cfg.token,
        }
    }
}

impl From<OneDriveStorageConfig> for StorageConfig {
    fn from(cfg: OneDriveStorageConfig) -> Self {
        StorageConfig::OneDrive {
            client_id: cfg.client_id,
            client_secret: cfg.client_secret,
            refresh_token: cfg.refresh_token,
            access_token: cfg.access_token,
            root_path: cfg.root_path,
        }
    }
}
