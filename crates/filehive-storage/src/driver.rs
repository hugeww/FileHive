use arc_swap::ArcSwap;
use bytes::Bytes;
use filehive_core::AppError;
use opendal::{services, Operator};
use std::path::Path;
use std::sync::Arc;

use crate::config::StorageConfig;

/// OpenDAL 统一文件存储驱动（支持运行时零锁热切换）
#[derive(Clone)]
pub struct StorageDriver {
    operator: Arc<ArcSwap<Operator>>,
    config: Arc<ArcSwap<StorageConfig>>,
}

/// 兼容别名
pub type FileStorage = StorageDriver;

/// 文件存储抽象接口
#[async_trait::async_trait]
pub trait FileStorageTrait: Send + Sync {
    async fn save_file(
        &self,
        path: &str,
        data: Bytes,
        content_type: Option<&str>,
    ) -> Result<(), AppError>;
    async fn read_file(&self, path: &str) -> Result<Bytes, AppError>;
    async fn read_file_range(&self, path: &str, range: std::ops::Range<u64>) -> Result<Bytes, AppError>;
    async fn reader(&self, path: &str) -> Result<opendal::Reader, AppError>;
    async fn delete_file(&self, path: &str) -> Result<(), AppError>;
    async fn file_exists(&self, path: &str) -> Result<bool, AppError>;
    async fn save_chunk(
        &self,
        upload_id: &str,
        chunk_index: i32,
        data: Bytes,
        save_path: &str,
    ) -> Result<(), AppError>;
    async fn merge_chunks(
        &self,
        upload_id: &str,
        total_chunks: i32,
        target_path: &str,
    ) -> Result<(), AppError>;
    async fn delete_chunks(&self, upload_id: &str) -> Result<(), AppError>;
    async fn generate_presigned_upload_url(
        &self,
        path: &str,
        expires_in_secs: u64,
    ) -> Result<Option<String>, AppError>;
}

impl StorageDriver {
    /// 根据配置初始化存储驱动
    pub fn new(config: &StorageConfig) -> Result<Self, AppError> {
        let operator = Self::build_operator(config)?;
        Ok(Self {
            operator: Arc::new(ArcSwap::from_pointee(operator)),
            config: Arc::new(ArcSwap::from_pointee(config.clone())),
        })
    }

    /// 从所有权配置创建存储驱动
    pub fn from_config(config: StorageConfig) -> Result<Self, AppError> {
        Self::new(&config)
    }

    /// 运行时热更新存储配置与底层驱动
    pub fn update(&self, config: &StorageConfig) -> Result<(), AppError> {
        let op = Self::build_operator(config)?;
        self.operator.store(Arc::new(op));
        self.config.store(Arc::new(config.clone()));
        Ok(())
    }

    /// 获取底层 OpenDAL Operator 守卫
    pub fn operator(&self) -> arc_swap::Guard<Arc<Operator>> {
        self.operator.load()
    }

    /// 获取当前存储配置守卫
    pub fn config(&self) -> arc_swap::Guard<Arc<StorageConfig>> {
        self.config.load()
    }

    /// 构建底层 OpenDAL Operator
    fn build_operator(config: &StorageConfig) -> Result<Operator, AppError> {
        let op = match config {
            StorageConfig::Local { storage_path } => {
                let mut builder = services::Fs::default();
                builder = builder.root(storage_path);
                Operator::new(builder)
                    .map_err(|e| AppError::StorageError {
                        status_code: 500,
                        detail: format!("Failed to build local storage operator: {}", e),
                    })?
                    .finish()
            }
            StorageConfig::S3 {
                endpoint,
                bucket,
                access_key_id,
                secret_access_key,
                region,
                root,
            } => {
                let mut builder = services::S3::default();
                builder = builder.bucket(bucket);
                if !endpoint.is_empty() {
                    builder = builder.endpoint(endpoint);
                }
                if !access_key_id.is_empty() {
                    builder = builder.access_key_id(access_key_id);
                }
                if !secret_access_key.is_empty() {
                    builder = builder.secret_access_key(secret_access_key);
                }
                if !region.is_empty() && region != "auto" {
                    builder = builder.region(region);
                }
                if let Some(r) = root {
                    if !r.is_empty() {
                        builder = builder.root(r);
                    }
                }
                Operator::new(builder)
                    .map_err(|e| AppError::StorageError {
                        status_code: 500,
                        detail: format!("Failed to build S3 storage operator: {}", e),
                    })?
                    .finish()
            }
            StorageConfig::WebDAV {
                endpoint,
                username,
                password,
                root_path,
                token,
            } => {
                let mut builder = services::Webdav::default();
                builder = builder.endpoint(endpoint);
                if !username.is_empty() {
                    builder = builder.username(username);
                }
                if !password.is_empty() {
                    builder = builder.password(password);
                }
                if !root_path.is_empty() {
                    builder = builder.root(root_path);
                }
                if let Some(tok) = token {
                    if !tok.is_empty() {
                        builder = builder.token(tok);
                    }
                }
                Operator::new(builder)
                    .map_err(|e| AppError::StorageError {
                        status_code: 500,
                        detail: format!("Failed to build WebDAV storage operator: {}", e),
                    })?
                    .finish()
            }
            StorageConfig::OneDrive {
                access_token,
                refresh_token,
                root_path,
                ..
            } => {
                let mut builder = services::Onedrive::default();
                if !root_path.is_empty() {
                    builder = builder.root(root_path);
                }
                let token = if !access_token.is_empty() {
                    access_token.as_str()
                } else {
                    refresh_token.as_str()
                };
                if !token.is_empty() {
                    builder = builder.access_token(token);
                }
                Operator::new(builder)
                    .map_err(|e| AppError::StorageError {
                        status_code: 500,
                        detail: format!("Failed to build OneDrive storage operator: {}", e),
                    })?
                    .finish()
            }
        };

        Ok(op)
    }

    /// 保存文件数据，支持可选的 Content-Type
    pub async fn save_file(
        &self,
        path: &str,
        data: Bytes,
        content_type: Option<&str>,
    ) -> Result<(), AppError> {
        let clean_path = path.trim_start_matches('/');
        let op = self.operator.load();
        if let Some(ct) = content_type {
            op.write_with(clean_path, data)
                .content_type(ct)
                .await
                .map_err(|e| AppError::StorageError {
                    status_code: 500,
                    detail: format!("Failed to write file {}: {}", path, e),
                })?;
        } else {
            op.write(clean_path, data)
                .await
                .map_err(|e| AppError::StorageError {
                    status_code: 500,
                    detail: format!("Failed to write file {}: {}", path, e),
                })?;
        }
        Ok(())
    }

    /// 简化的保存文件方法（无指定 Content-Type）
    pub async fn save_file_bytes(&self, path: &str, data: Bytes) -> Result<(), AppError> {
        self.save_file(path, data, None).await
    }

    /// 读取完整文件内容
    pub async fn read_file(&self, path: &str) -> Result<Bytes, AppError> {
        let clean_path = path.trim_start_matches('/');
        let buf = self
            .operator
            .load()
            .read(clean_path)
            .await
            .map_err(|e| match e.kind() {
                opendal::ErrorKind::NotFound => {
                    AppError::NotFound(format!("File not found {}: {}", path, e))
                }
                opendal::ErrorKind::PermissionDenied => {
                    AppError::Forbidden(format!("Permission denied for file {}: {}", path, e))
                }
                _ => AppError::StorageError {
                    status_code: 500,
                    detail: format!("Failed to read file {}: {}", path, e),
                },
            })?;
        Ok(buf.to_bytes())
    }

    /// 分段读取文件内容 (支持 HTTP Range 断点续传)
    pub async fn read_file_range(&self, path: &str, range: std::ops::Range<u64>) -> Result<Bytes, AppError> {
        let clean_path = path.trim_start_matches('/');
        let buf = self
            .operator
            .load()
            .read_with(clean_path)
            .range(range)
            .await
            .map_err(|e| match e.kind() {
                opendal::ErrorKind::NotFound => {
                    AppError::NotFound(format!("File not found {}: {}", path, e))
                }
                opendal::ErrorKind::PermissionDenied => {
                    AppError::Forbidden(format!("Permission denied for file {}: {}", path, e))
                }
                _ => AppError::StorageError {
                    status_code: 500,
                    detail: format!("Failed to read range for {}: {}", path, e),
                },
            })?;
        Ok(buf.to_bytes())
    }

    /// 获取文件的流式读取器 Reader
    pub async fn reader(&self, path: &str) -> Result<opendal::Reader, AppError> {
        let clean_path = path.trim_start_matches('/');
        let r = self
            .operator
            .load()
            .reader(clean_path)
            .await
            .map_err(|e| match e.kind() {
                opendal::ErrorKind::NotFound => {
                    AppError::NotFound(format!("File not found {}: {}", path, e))
                }
                opendal::ErrorKind::PermissionDenied => {
                    AppError::Forbidden(format!("Permission denied for file {}: {}", path, e))
                }
                _ => AppError::StorageError {
                    status_code: 500,
                    detail: format!("Failed to get reader for {}: {}", path, e),
                },
            })?;
        Ok(r)
    }

    /// 删除指定路径的文件
    pub async fn delete_file(&self, path: &str) -> Result<(), AppError> {
        let clean_path = path.trim_start_matches('/');
        self.operator
            .load()
            .delete(clean_path)
            .await
            .map_err(|e| AppError::StorageError {
                status_code: 500,
                detail: format!("Failed to delete file {}: {}", path, e),
            })?;
        Ok(())
    }

    /// 删除 StoredFile 模型对应的文件
    pub async fn delete_stored_file(&self, file: &filehive_core::StoredFile) -> Result<(), AppError> {
        self.delete_file(&file.get_file_path()).await
    }

    /// 检查指定路径的文件是否存在
    pub async fn file_exists(&self, path: &str) -> Result<bool, AppError> {
        let clean_path = path.trim_start_matches('/');
        self.operator
            .load()
            .exists(clean_path)
            .await
            .map_err(|e| AppError::StorageError {
                status_code: 500,
                detail: format!("Failed to check file existence for {}: {}", path, e),
            })
    }

    /// 计算分片存储路径
    fn resolve_chunk_path(upload_id: &str, chunk_index: i32, save_path: &str) -> String {
        if save_path.is_empty() {
            format!("chunks/{}/{}.part", upload_id, chunk_index)
        } else if save_path.ends_with(".part") {
            save_path.to_string()
        } else {
            let p = Path::new(save_path);
            if let Some(parent) = p.parent() {
                let parent_str = parent.to_string_lossy();
                if !parent_str.is_empty() && parent_str != "." {
                    format!("{}/chunks/{}/{}.part", parent_str, upload_id, chunk_index)
                } else {
                    format!("chunks/{}/{}.part", upload_id, chunk_index)
                }
            } else {
                format!("chunks/{}/{}.part", upload_id, chunk_index)
            }
        }
    }

    /// 定位已保存分片的读取路径
    async fn find_chunk_path(
        &self,
        upload_id: &str,
        chunk_index: i32,
        target_path: &str,
    ) -> Result<String, AppError> {
        let clean_target = target_path.trim_start_matches('/');
        let p = Path::new(clean_target);
        if let Some(parent) = p.parent() {
            let parent_str = parent.to_string_lossy();
            if !parent_str.is_empty() && parent_str != "." {
                let candidate = format!("{}/chunks/{}/{}.part", parent_str, upload_id, chunk_index);
                if self.file_exists(&candidate).await.unwrap_or(false) {
                    return Ok(candidate);
                }
            }
        }

        let candidate = format!("chunks/{}/{}.part", upload_id, chunk_index);
        if self.file_exists(&candidate).await.unwrap_or(false) {
            return Ok(candidate);
        }

        Ok(candidate)
    }

    /// 保存单个分片数据
    pub async fn save_chunk(
        &self,
        upload_id: &str,
        chunk_index: i32,
        data: Bytes,
        save_path: &str,
    ) -> Result<(), AppError> {
        let chunk_path = Self::resolve_chunk_path(upload_id, chunk_index, save_path);
        self.save_file(&chunk_path, data, None).await
    }

    /// 合并所有分片到目标文件
    pub async fn merge_chunks(
        &self,
        upload_id: &str,
        total_chunks: i32,
        target_path: &str,
    ) -> Result<(), AppError> {
        if total_chunks <= 0 {
            return Err(AppError::BadRequest(
                "total_chunks must be greater than 0".to_string(),
            ));
        }

        let clean_target = target_path.trim_start_matches('/');
        let mut writer = self
            .operator
            .load()
            .writer(clean_target)
            .await
            .map_err(|e| AppError::StorageError {
                status_code: 500,
                detail: format!("Failed to open writer for merge target {}: {}", target_path, e),
            })?;

        for i in 0..total_chunks {
            let chunk_path = self.find_chunk_path(upload_id, i, target_path).await?;
            let chunk_data = self.read_file(&chunk_path).await?;
            writer
                .write(chunk_data)
                .await
                .map_err(|e| AppError::StorageError {
                    status_code: 500,
                    detail: format!("Failed to write chunk {} during merge: {}", i, e),
                })?;
        }

        writer.close().await.map_err(|e| AppError::StorageError {
            status_code: 500,
            detail: format!("Failed to complete write for {}: {}", target_path, e),
        })?;

        Ok(())
    }

    /// 清理 upload_id 对应的所有分片文件
    pub async fn delete_chunks(&self, upload_id: &str) -> Result<(), AppError> {
        let prefix = format!("chunks/{}/", upload_id);
        self.operator
            .load()
            .remove_all(&prefix)
            .await
            .map_err(|e| AppError::StorageError {
                status_code: 500,
                detail: format!("Failed to remove chunks for upload_id {}: {}", upload_id, e),
            })?;
        Ok(())
    }

    /// 针对特定 save_path 深度清理分片
    pub async fn delete_chunks_at(&self, upload_id: &str, save_path: &str) -> Result<(), AppError> {
        let _ = self.delete_chunks(upload_id).await;

        if !save_path.is_empty() {
            let clean_save = save_path.trim_start_matches('/');
            let p = Path::new(clean_save);
            if let Some(parent) = p.parent() {
                let parent_str = parent.to_string_lossy();
                if !parent_str.is_empty() && parent_str != "." {
                    let sub_prefix = format!("{}/chunks/{}/", parent_str, upload_id);
                    let _ = self.operator.load().remove_all(&sub_prefix).await;
                }
            }
        }
        Ok(())
    }

    /// 生成预签名上传 URL（S3 存储后端支持）
    pub async fn generate_presigned_upload_url(
        &self,
        path: &str,
        expires_in_secs: u64,
    ) -> Result<Option<String>, AppError> {
        let clean_path = path.trim_start_matches('/');
        let duration = std::time::Duration::from_secs(expires_in_secs);
        match self.operator.load().presign_write(clean_path, duration).await {
            Ok(signed_req) => Ok(Some(signed_req.uri().to_string())),
            Err(_) => Ok(None),
        }
    }
}

#[async_trait::async_trait]
impl FileStorageTrait for StorageDriver {
    async fn save_file(
        &self,
        path: &str,
        data: Bytes,
        content_type: Option<&str>,
    ) -> Result<(), AppError> {
        self.save_file(path, data, content_type).await
    }

    async fn read_file(&self, path: &str) -> Result<Bytes, AppError> {
        self.read_file(path).await
    }

    async fn read_file_range(&self, path: &str, range: std::ops::Range<u64>) -> Result<Bytes, AppError> {
        self.read_file_range(path, range).await
    }

    async fn reader(&self, path: &str) -> Result<opendal::Reader, AppError> {
        self.reader(path).await
    }

    async fn delete_file(&self, path: &str) -> Result<(), AppError> {
        self.delete_file(path).await
    }

    async fn file_exists(&self, path: &str) -> Result<bool, AppError> {
        self.file_exists(path).await
    }

    async fn save_chunk(
        &self,
        upload_id: &str,
        chunk_index: i32,
        data: Bytes,
        save_path: &str,
    ) -> Result<(), AppError> {
        self.save_chunk(upload_id, chunk_index, data, save_path).await
    }

    async fn merge_chunks(
        &self,
        upload_id: &str,
        total_chunks: i32,
        target_path: &str,
    ) -> Result<(), AppError> {
        self.merge_chunks(upload_id, total_chunks, target_path).await
    }

    async fn delete_chunks(&self, upload_id: &str) -> Result<(), AppError> {
        self.delete_chunks(upload_id).await
    }

    async fn generate_presigned_upload_url(
        &self,
        path: &str,
        expires_in_secs: u64,
    ) -> Result<Option<String>, AppError> {
        self.generate_presigned_upload_url(path, expires_in_secs).await
    }
}
