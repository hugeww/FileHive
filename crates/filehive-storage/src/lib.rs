pub mod config;
pub mod driver;

pub use config::{
    default_local_path, default_onedrive_root, default_s3_region, default_webdav_root,
    LocalStorageConfig, OneDriveStorageConfig, S3StorageConfig, StorageConfig, WebDAVStorageConfig,
};
pub use driver::{FileStorage, FileStorageTrait, StorageDriver};

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn create_temp_local_driver() -> (StorageDriver, std::path::PathBuf) {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!("filehive_storage_test_{}_{}", std::process::id(), counter));
        let _ = std::fs::create_dir_all(&temp_dir);

        let config = StorageConfig::Local {
            storage_path: temp_dir.to_str().unwrap().to_string(),
        };

        let storage = StorageDriver::new(&config).unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_local_storage_roundtrip() {
        let (storage, temp_dir) = create_temp_local_driver();
        let payload = Bytes::from("hello opendal");

        // 1. 保存文件（带 content_type）
        storage
            .save_file("test.txt", payload.clone(), Some("text/plain"))
            .await
            .unwrap();

        // 2. 存在性检查
        assert!(storage.file_exists("test.txt").await.unwrap());

        // 3. 读取文件
        let read = storage.read_file("test.txt").await.unwrap();
        assert_eq!(read, payload);

        // 4. 测试 reader
        let r = storage.reader("test.txt").await.unwrap();
        let read_buf = r.read(..).await.unwrap();
        assert_eq!(read_buf.to_bytes(), payload);

        // 4.1 测试 read_file_range
        let range_buf = storage.read_file_range("test.txt", 0..5).await.unwrap();
        assert_eq!(range_buf, Bytes::from("hello"));

        // 5. 删除文件
        storage.delete_file("test.txt").await.unwrap();
        assert!(!storage.file_exists("test.txt").await.unwrap());

        // 6. 读取已删除文件应返回 404 / NotFound
        let err = storage.read_file("test.txt").await.unwrap_err();
        assert_eq!(err.status_code(), 404);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_chunk_upload_and_merge() {
        let (storage, temp_dir) = create_temp_local_driver();
        let upload_id = "test_upload_123";

        let chunk0 = Bytes::from("chunk-part-0;");
        let chunk1 = Bytes::from("chunk-part-1;");
        let chunk2 = Bytes::from("chunk-part-2;");

        // 保存分片
        storage.save_chunk(upload_id, 0, chunk0.clone(), "").await.unwrap();
        storage.save_chunk(upload_id, 1, chunk1.clone(), "").await.unwrap();
        storage.save_chunk(upload_id, 2, chunk2.clone(), "").await.unwrap();

        // 合并分片
        let merged_path = "merged_result.txt";
        storage.merge_chunks(upload_id, 3, merged_path).await.unwrap();

        // 验证合并内容
        let content = storage.read_file(merged_path).await.unwrap();
        let expected = Bytes::from("chunk-part-0;chunk-part-1;chunk-part-2;");
        assert_eq!(content, expected);

        // 清理分片
        storage.delete_chunks(upload_id).await.unwrap();
        // 清理合并文件
        storage.delete_file(merged_path).await.unwrap();

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_chunk_upload_with_subpath() {
        let (storage, temp_dir) = create_temp_local_driver();
        let upload_id = "test_upload_subpath";
        let target_file = "uploads/2026/test_target.bin";

        let chunk0 = Bytes::from("AAA");
        let chunk1 = Bytes::from("BBB");

        // 带 save_path 保存分片
        storage.save_chunk(upload_id, 0, chunk0.clone(), target_file).await.unwrap();
        storage.save_chunk(upload_id, 1, chunk1.clone(), target_file).await.unwrap();

        // 合并到目标路径
        storage.merge_chunks(upload_id, 2, target_file).await.unwrap();

        let content = storage.read_file(target_file).await.unwrap();
        assert_eq!(content, Bytes::from("AAABBB"));

        // 清理
        storage.delete_chunks_at(upload_id, target_file).await.unwrap();
        storage.delete_file(target_file).await.unwrap();

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_storage_config_serde() {
        // 1. Local
        let json_local = r#"{"type":"local","storage_path":"/tmp/share"}"#;
        let cfg: StorageConfig = serde_json::from_str(json_local).unwrap();
        match cfg {
            StorageConfig::Local { storage_path } => assert_eq!(storage_path, "/tmp/share"),
            _ => panic!("Expected Local variant"),
        }

        // 2. S3
        let json_s3 = r#"{
            "type": "s3",
            "endpoint": "https://s3.example.com",
            "bucket": "my-bucket",
            "access_key_id": "ak",
            "secret_access_key": "sk",
            "region": "us-east-1"
        }"#;
        let cfg_s3: StorageConfig = serde_json::from_str(json_s3).unwrap();
        match cfg_s3 {
            StorageConfig::S3 { bucket, region, .. } => {
                assert_eq!(bucket, "my-bucket");
                assert_eq!(region, "us-east-1");
            }
            _ => panic!("Expected S3 variant"),
        }

        // 3. WebDAV
        let json_webdav = r#"{
            "type": "webdav",
            "endpoint": "https://dav.example.com",
            "username": "user",
            "password": "pwd",
            "root_path": "files"
        }"#;
        let cfg_dav: StorageConfig = serde_json::from_str(json_webdav).unwrap();
        match cfg_dav {
            StorageConfig::WebDAV { endpoint, username, .. } => {
                assert_eq!(endpoint, "https://dav.example.com");
                assert_eq!(username, "user");
            }
            _ => panic!("Expected WebDAV variant"),
        }

        // 4. OneDrive
        let json_onedrive = r#"{
            "type": "onedrive",
            "access_token": "token123",
            "root_path": "box"
        }"#;
        let cfg_od: StorageConfig = serde_json::from_str(json_onedrive).unwrap();
        match cfg_od {
            StorageConfig::OneDrive { access_token, root_path, .. } => {
                assert_eq!(access_token, "token123");
                assert_eq!(root_path, "box");
            }
            _ => panic!("Expected OneDrive variant"),
        }
    }
}
