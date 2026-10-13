use std::time::Duration;
use chrono::Utc;
use fcb_core::models::StoredFile;
use fcb_db::crud::{
    delete_expired_presign_sessions, delete_expired_reservations, delete_file_code,
    get_expired_file_codes,
};
use fcb_db::DbPool;
use fcb_storage::FileStorage;
use tracing::{error, info};

pub async fn start_background_tasks(pool: DbPool, storage: FileStorage) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(600)).await;
            let now = Utc::now();

            // 1. 清理过期文件
            match get_expired_file_codes(&pool, now).await {
                Ok(expired_files) => {
                    for f in expired_files {
                        // 如果不是本地目录挂载 (local-ref)
                        if f.file_path.as_deref() != Some("local-ref") {
                            let stored = StoredFile {
                                file_path: f.file_path.unwrap_or_default(),
                                uuid_file_name: f.uuid_file_name.unwrap_or_default(),
                                code: f.code.clone(),
                                prefix: f.prefix,
                                suffix: f.suffix,
                                text: f.text,
                            };
                            let full_path = stored.get_file_path();
                            if !full_path.is_empty() {
                                let _ = storage.delete_file(&full_path).await;
                            }
                        }
                        if let Err(e) = delete_file_code(&pool, f.id).await {
                            error!("Failed to delete expired file code {}: {}", f.code, e);
                        }
                    }
                }
                Err(e) => error!("Failed to query expired files: {}", e),
            }

            // 2. 清理过期的容量预留
            let _ = delete_expired_reservations(&pool, now).await;

            // 3. 清理过期的预签名会话
            let _ = delete_expired_presign_sessions(&pool, now).await;
        }
    });
    info!("Background cleanup tasks started");
}
