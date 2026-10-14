use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Model for the `filecodes` table.
/// Corresponds to Python `apps.base.models.FileCodes`.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct FileCode {
    pub id: i64,
    pub code: String,
    pub prefix: String,
    pub suffix: String,
    pub uuid_file_name: Option<String>,
    pub file_path: Option<String>,
    pub size: i64,
    pub text: Option<String>,
    pub expired_at: Option<DateTime<Utc>>,
    /// -1 = time-based expiration (evaluated via expired_at).
    /// <= 0 (specifically 0) = count-based expiration (expired when 0).
    pub expired_count: i32,
    pub used_count: i32,
    pub created_at: Option<DateTime<Utc>>,
    pub file_hash: Option<String>,
    pub is_chunked: bool,
    pub upload_id: Option<String>,
}

impl FileCode {
    /// Checks whether the file code is expired according to Python `FileCodes.is_expired`:
    /// ```python
    /// if self.expired_at is None:
    ///     return False
    /// if self.expired_at and self.expired_count < 0:
    ///     return self.expired_at < await get_now()
    /// return self.expired_count <= 0
    /// ```
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        if self.expired_at.is_none() {
            return false;
        }
        if let Some(exp) = self.expired_at {
            if self.expired_count < 0 {
                return exp < now;
            }
        }
        self.expired_count <= 0
    }

    /// Helper to get the concatenated stored file path (`file_path/uuid_file_name`).
    pub fn get_file_path(&self) -> String {
        match (&self.file_path, &self.uuid_file_name) {
            (Some(dir), Some(name)) if !dir.is_empty() => format!("{}/{}", dir, name),
            (_, Some(name)) => name.clone(),
            _ => String::new(),
        }
    }
}

/// Model for the `uploadchunk` table.
/// Corresponds to Python `apps.base.models.UploadChunk`.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct UploadChunk {
    pub id: i64,
    pub upload_id: String,
    pub chunk_index: i32,
    pub chunk_hash: String,
    pub total_chunks: i32,
    pub file_size: i64,
    pub chunk_size: i32,
    pub file_name: String,
    pub save_path: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub completed: bool,
}

/// Model for the `keyvalue` table.
/// Corresponds to Python `apps.base.models.KeyValue`.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct KeyValue {
    pub id: i64,
    pub key: String,
    pub value: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Model for the `presignuploadsession` table.
/// Corresponds to Python `apps.base.models.PresignUploadSession`.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct PresignSession {
    pub id: i64,
    pub upload_id: String,
    pub file_name: String,
    pub file_size: i64,
    pub save_path: String,
    pub mode: String,
    pub expire_value: i32,
    pub expire_style: String,
    pub created_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
}

impl PresignSession {
    /// Checks whether the presigned session has expired.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at < now
    }
}

/// Model for the `storagereservation` table.
/// Corresponds to Python `apps.base.models.StorageReservation`.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct StorageReservation {
    pub id: i64,
    pub token: String,
    pub size: i64,
    pub expires_at: DateTime<Utc>,
}

impl StorageReservation {
    /// Checks whether the storage reservation has expired.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at <= now
    }
}
