use chrono::{DateTime, Utc};
use fcb_core::AppError;
use sqlx::SqlitePool;

use crate::models::{FileCode, KeyValue, PresignSession, StorageReservation, UploadChunk};

// ============================================================================
// FileCodes CRUD
// ============================================================================

/// Parameters for inserting a new FileCode.
#[derive(Debug, Clone, Default)]
pub struct NewFileCode {
    pub code: String,
    pub prefix: String,
    pub suffix: String,
    pub uuid_file_name: Option<String>,
    pub file_path: Option<String>,
    pub size: i64,
    pub text: Option<String>,
    pub expired_at: Option<DateTime<Utc>>,
    pub expired_count: i32,
    pub used_count: i32,
    pub file_hash: Option<String>,
    pub is_chunked: bool,
    pub upload_id: Option<String>,
}

/// Create a new filecode record.
pub async fn insert_file_code(
    pool: &SqlitePool,
    new_code: &NewFileCode,
) -> Result<FileCode, AppError> {
    let row = sqlx::query_as::<_, FileCode>(
        r#"
        INSERT INTO filecodes (
            code, prefix, suffix, uuid_file_name, file_path, size, text,
            expired_at, expired_count, used_count, file_hash, is_chunked, upload_id
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        RETURNING id, code, prefix, suffix, uuid_file_name, file_path, size, text,
                  expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
        "#,
    )
    .bind(&new_code.code)
    .bind(&new_code.prefix)
    .bind(&new_code.suffix)
    .bind(&new_code.uuid_file_name)
    .bind(&new_code.file_path)
    .bind(new_code.size)
    .bind(&new_code.text)
    .bind(new_code.expired_at)
    .bind(new_code.expired_count)
    .bind(new_code.used_count)
    .bind(&new_code.file_hash)
    .bind(new_code.is_chunked)
    .bind(&new_code.upload_id)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to insert file code: {}", e)))?;

    Ok(row)
}

/// Get a file code by its unique pickup code.
pub async fn get_file_code_by_code(
    pool: &SqlitePool,
    code: &str,
) -> Result<Option<FileCode>, AppError> {
    sqlx::query_as::<_, FileCode>(
        r#"
        SELECT id, code, prefix, suffix, uuid_file_name, file_path, size, text,
               expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
        FROM filecodes
        WHERE code = ?
        "#,
    )
    .bind(code)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch file code by code: {}", e)))
}

/// Get a file code by its ID.
pub async fn get_file_code_by_id(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<FileCode>, AppError> {
    sqlx::query_as::<_, FileCode>(
        r#"
        SELECT id, code, prefix, suffix, uuid_file_name, file_path, size, text,
               expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
        FROM filecodes
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch file code by id: {}", e)))
}

/// Atomically consume/use a filecode if valid and not expired.
///
/// Corresponds to Python logic in views.py:
/// ```python
/// eligible = Q(expired_count=-1) | Q(expired_count__gt=0)
/// updated = await FileCodes.filter(Q(id=file_code.id) & eligible).update(
///     used_count=F("used_count") + 1,
///     expired_count=Case(
///         When(expired_count__gt=0, then=F("expired_count") - 1),
///         default=F("expired_count")
///     )
/// )
/// ```
/// Returns the updated `FileCode` if eligible and updated, or `None` if not eligible (or expired count reached 0).
pub async fn consume_file_code(
    pool: &SqlitePool,
    id: i64,
    now: DateTime<Utc>,
) -> Result<Option<FileCode>, AppError> {
    let row = sqlx::query_as::<_, FileCode>(
        r#"
        UPDATE filecodes
        SET used_count = used_count + 1,
            expired_count = CASE
                WHEN expired_count > 0 THEN expired_count - 1
                ELSE expired_count
            END
        WHERE id = ?
          AND (expired_count = -1 OR expired_count > 0)
          AND (expired_at IS NULL OR expired_count > 0 OR expired_at >= ?)
        RETURNING id, code, prefix, suffix, uuid_file_name, file_path, size, text,
                  expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
        "#,
    )
    .bind(id)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to consume file code: {}", e)))?;

    Ok(row)
}

/// Delete a file code by its ID. Returns true if a row was deleted.
pub async fn delete_file_code(pool: &SqlitePool, id: i64) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM filecodes WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete file code: {}", e)))?;

    Ok(result.rows_affected() > 0)
}

/// Find all expired filecodes for background cleanup.
/// Expiration condition matches Python:
/// - (expired_at is not null and expired_count < 0 and expired_at < now) OR
/// - (expired_count == 0)
pub async fn get_expired_file_codes(
    pool: &SqlitePool,
    now: DateTime<Utc>,
) -> Result<Vec<FileCode>, AppError> {
    sqlx::query_as::<_, FileCode>(
        r#"
        SELECT id, code, prefix, suffix, uuid_file_name, file_path, size, text,
               expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
        FROM filecodes
        WHERE (expired_at IS NOT NULL AND expired_count < 0 AND expired_at < ?)
           OR (expired_count = 0)
        "#,
    )
    .bind(now)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to list expired file codes: {}", e)))
}

/// Count total stored filecodes.
pub async fn count_total_files(pool: &SqlitePool) -> Result<i64, AppError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM filecodes")
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to count file codes: {}", e)))?;

    Ok(row.0)
}

/// Total used storage size of non-text files.
pub async fn sum_total_storage_size(pool: &SqlitePool) -> Result<i64, AppError> {
    let row: (Option<i64>,) = sqlx::query_as("SELECT SUM(size) FROM filecodes WHERE text IS NULL")
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to sum storage size: {}", e)))?;

    Ok(row.0.unwrap_or(0))
}

// ============================================================================
// KeyValue CRUD
// ============================================================================

/// Get a key-value record by key.
pub async fn get_key_value(pool: &SqlitePool, key: &str) -> Result<Option<KeyValue>, AppError> {
    sqlx::query_as::<_, KeyValue>(
        r#"
        SELECT id, key, value, created_at
        FROM keyvalue
        WHERE key = ?
        "#,
    )
    .bind(key)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to get key value for key '{}': {}", key, e)))
}

/// Set or update a key-value record (upsert).
pub async fn set_key_value(
    pool: &SqlitePool,
    key: &str,
    value: Option<&str>,
) -> Result<KeyValue, AppError> {
    let row = sqlx::query_as::<_, KeyValue>(
        r#"
        INSERT INTO keyvalue (key, value)
        VALUES (?, ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value
        RETURNING id, key, value, created_at
        "#,
    )
    .bind(key)
    .bind(value)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to set key value for key '{}': {}", key, e)))?;

    Ok(row)
}

/// Delete a key-value record by key.
pub async fn delete_key_value(pool: &SqlitePool, key: &str) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM keyvalue WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete key '{}': {}", key, e)))?;

    Ok(result.rows_affected() > 0)
}

// ============================================================================
// StorageReservation CRUD
// ============================================================================

/// Insert or update a storage reservation.
pub async fn insert_or_update_reservation(
    pool: &SqlitePool,
    token: &str,
    size: i64,
    expires_at: DateTime<Utc>,
) -> Result<StorageReservation, AppError> {
    let row = sqlx::query_as::<_, StorageReservation>(
        r#"
        INSERT INTO storagereservation (token, size, expires_at)
        VALUES (?, ?, ?)
        ON CONFLICT(token) DO UPDATE SET size = excluded.size, expires_at = excluded.expires_at
        RETURNING id, token, size, expires_at
        "#,
    )
    .bind(token)
    .bind(size)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to upsert storage reservation: {}", e)))?;

    Ok(row)
}

/// Get an active (unexpired) storage reservation by token.
pub async fn get_active_reservation(
    pool: &SqlitePool,
    token: &str,
    now: DateTime<Utc>,
) -> Result<Option<StorageReservation>, AppError> {
    sqlx::query_as::<_, StorageReservation>(
        r#"
        SELECT id, token, size, expires_at
        FROM storagereservation
        WHERE token = ? AND expires_at > ?
        "#,
    )
    .bind(token)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch active reservation: {}", e)))
}

/// Delete storage reservation by token.
pub async fn delete_reservation(pool: &SqlitePool, token: &str) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM storagereservation WHERE token = ?")
        .bind(token)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete reservation: {}", e)))?;

    Ok(result.rows_affected() > 0)
}

/// Clean up expired storage reservations.
pub async fn delete_expired_reservations(
    pool: &SqlitePool,
    now: DateTime<Utc>,
) -> Result<u64, AppError> {
    let result = sqlx::query("DELETE FROM storagereservation WHERE expires_at <= ?")
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to purge expired reservations: {}", e)))?;

    Ok(result.rows_affected())
}

/// Sum the total active reserved storage size.
pub async fn sum_active_reserved_size(
    pool: &SqlitePool,
    now: DateTime<Utc>,
) -> Result<i64, AppError> {
    let row: (Option<i64>,) = sqlx::query_as(
        "SELECT SUM(size) FROM storagereservation WHERE expires_at > ?",
    )
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to sum active reserved size: {}", e)))?;

    Ok(row.0.unwrap_or(0))
}

// ============================================================================
// UploadChunk CRUD
// ============================================================================

/// Insert or update an upload chunk.
pub async fn upsert_upload_chunk(
    pool: &SqlitePool,
    chunk: &UploadChunk,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO uploadchunk (
            upload_id, chunk_index, chunk_hash, total_chunks,
            file_size, chunk_size, file_name, save_path, completed
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&chunk.upload_id)
    .bind(chunk.chunk_index)
    .bind(&chunk.chunk_hash)
    .bind(chunk.total_chunks)
    .bind(chunk.file_size)
    .bind(chunk.chunk_size)
    .bind(&chunk.file_name)
    .bind(&chunk.save_path)
    .bind(chunk.completed)
    .execute(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to insert upload chunk: {}", e)))?;

    Ok(())
}

/// Get all chunks for a given upload_id.
pub async fn get_upload_chunks(
    pool: &SqlitePool,
    upload_id: &str,
) -> Result<Vec<UploadChunk>, AppError> {
    sqlx::query_as::<_, UploadChunk>(
        r#"
        SELECT id, upload_id, chunk_index, chunk_hash, total_chunks,
               file_size, chunk_size, file_name, save_path, created_at, completed
        FROM uploadchunk
        WHERE upload_id = ?
        ORDER BY chunk_index ASC
        "#,
    )
    .bind(upload_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch upload chunks: {}", e)))
}

/// Get a specific upload chunk by upload_id and chunk_index.
pub async fn get_upload_chunk(
    pool: &SqlitePool,
    upload_id: &str,
    chunk_index: i32,
) -> Result<Option<UploadChunk>, AppError> {
    sqlx::query_as::<_, UploadChunk>(
        r#"
        SELECT id, upload_id, chunk_index, chunk_hash, total_chunks,
               file_size, chunk_size, file_name, save_path, created_at, completed
        FROM uploadchunk
        WHERE upload_id = ? AND chunk_index = ?
        LIMIT 1
        "#,
    )
    .bind(upload_id)
    .bind(chunk_index)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch upload chunk: {}", e)))
}

/// Save or update an upload chunk (updates if (upload_id, chunk_index) exists, else inserts).
pub async fn save_or_update_upload_chunk(
    pool: &SqlitePool,
    chunk: &UploadChunk,
) -> Result<(), AppError> {
    let existing = get_upload_chunk(pool, &chunk.upload_id, chunk.chunk_index).await?;
    if existing.is_some() {
        sqlx::query(
            r#"
            UPDATE uploadchunk
            SET chunk_hash = ?, total_chunks = ?, file_size = ?,
                chunk_size = ?, file_name = ?, save_path = ?, completed = ?
            WHERE upload_id = ? AND chunk_index = ?
            "#,
        )
        .bind(&chunk.chunk_hash)
        .bind(chunk.total_chunks)
        .bind(chunk.file_size)
        .bind(chunk.chunk_size)
        .bind(&chunk.file_name)
        .bind(&chunk.save_path)
        .bind(chunk.completed)
        .bind(&chunk.upload_id)
        .bind(chunk.chunk_index)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to update upload chunk: {}", e)))?;
    } else {
        upsert_upload_chunk(pool, chunk).await?;
    }
    Ok(())
}

/// Get all completed chunks (chunk_index >= 0 and completed = 1) for a given upload_id.
pub async fn get_completed_upload_chunks(
    pool: &SqlitePool,
    upload_id: &str,
) -> Result<Vec<UploadChunk>, AppError> {
    sqlx::query_as::<_, UploadChunk>(
        r#"
        SELECT id, upload_id, chunk_index, chunk_hash, total_chunks,
               file_size, chunk_size, file_name, save_path, created_at, completed
        FROM uploadchunk
        WHERE upload_id = ? AND chunk_index >= 0 AND completed = 1
        ORDER BY chunk_index ASC
        "#,
    )
    .bind(upload_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch completed upload chunks: {}", e)))
}

/// Count completed chunks for a given upload_id.
pub async fn count_completed_chunks(
    pool: &SqlitePool,
    upload_id: &str,
) -> Result<i64, AppError> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM uploadchunk WHERE upload_id = ? AND chunk_index >= 0 AND completed = 1"
    )
    .bind(upload_id)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to count completed chunks: {}", e)))?;
    Ok(row.0)
}

/// Find an existing incomplete chunk session for breakpoint resume.
pub async fn find_existing_chunk_session(
    pool: &SqlitePool,
    file_hash: &str,
    file_size: i64,
    file_name: &str,
) -> Result<Option<UploadChunk>, AppError> {
    sqlx::query_as::<_, UploadChunk>(
        r#"
        SELECT id, upload_id, chunk_index, chunk_hash, total_chunks,
               file_size, chunk_size, file_name, save_path, created_at, completed
        FROM uploadchunk
        WHERE chunk_hash = ? AND chunk_index = -1 AND file_size = ? AND file_name = ?
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(file_hash)
    .bind(file_size)
    .bind(file_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to find existing chunk session: {}", e)))
}

/// Delete all chunks associated with an upload_id.
pub async fn delete_upload_chunks(pool: &SqlitePool, upload_id: &str) -> Result<u64, AppError> {
    let res = sqlx::query("DELETE FROM uploadchunk WHERE upload_id = ?")
        .bind(upload_id)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete upload chunks: {}", e)))?;

    Ok(res.rows_affected())
}

// ============================================================================
// PresignSession CRUD
// ============================================================================

/// Insert a new presign upload session.
pub async fn insert_presign_session(
    pool: &SqlitePool,
    session: &PresignSession,
) -> Result<PresignSession, AppError> {
    let row = sqlx::query_as::<_, PresignSession>(
        r#"
        INSERT INTO presignuploadsession (
            upload_id, file_name, file_size, save_path, mode,
            expire_value, expire_style, expires_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        RETURNING id, upload_id, file_name, file_size, save_path, mode,
                  expire_value, expire_style, created_at, expires_at
        "#,
    )
    .bind(&session.upload_id)
    .bind(&session.file_name)
    .bind(session.file_size)
    .bind(&session.save_path)
    .bind(&session.mode)
    .bind(session.expire_value)
    .bind(&session.expire_style)
    .bind(session.expires_at)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to insert presign session: {}", e)))?;

    Ok(row)
}

/// Get a presign upload session by its upload_id.
pub async fn get_presign_session(
    pool: &SqlitePool,
    upload_id: &str,
) -> Result<Option<PresignSession>, AppError> {
    sqlx::query_as::<_, PresignSession>(
        r#"
        SELECT id, upload_id, file_name, file_size, save_path, mode,
               expire_value, expire_style, created_at, expires_at
        FROM presignuploadsession
        WHERE upload_id = ?
        "#,
    )
    .bind(upload_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch presign session: {}", e)))
}

/// Delete a presign upload session by its upload_id.
pub async fn delete_presign_session(
    pool: &SqlitePool,
    upload_id: &str,
) -> Result<bool, AppError> {
    let res = sqlx::query("DELETE FROM presignuploadsession WHERE upload_id = ?")
        .bind(upload_id)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete presign session: {}", e)))?;

    Ok(res.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_pool;
    use chrono::Duration;

    #[tokio::test]
    async fn test_filecode_crud_and_consumption() {
        let pool = init_pool(":memory:").await.unwrap();
        let now = Utc::now();

        let new_code = NewFileCode {
            code: "12345".to_string(),
            prefix: "pre".to_string(),
            suffix: "suf".to_string(),
            uuid_file_name: Some("test.txt".to_string()),
            file_path: Some("uploads".to_string()),
            size: 1024,
            text: None,
            expired_at: Some(now + Duration::hours(1)),
            expired_count: 2,
            used_count: 0,
            file_hash: Some("hash123".to_string()),
            is_chunked: false,
            upload_id: None,
        };

        // Insert
        let fc = insert_file_code(&pool, &new_code).await.unwrap();
        assert_eq!(fc.code, "12345");
        assert_eq!(fc.used_count, 0);
        assert_eq!(fc.expired_count, 2);
        assert_eq!(fc.get_file_path(), "uploads/test.txt");

        // Fetch by code
        let fetched = get_file_code_by_code(&pool, "12345").await.unwrap().unwrap();
        assert_eq!(fetched.id, fc.id);

        // Consume once
        let consumed1 = consume_file_code(&pool, fc.id, now).await.unwrap().unwrap();
        assert_eq!(consumed1.used_count, 1);
        assert_eq!(consumed1.expired_count, 1);

        // Consume twice
        let consumed2 = consume_file_code(&pool, fc.id, now).await.unwrap().unwrap();
        assert_eq!(consumed2.used_count, 2);
        assert_eq!(consumed2.expired_count, 0);

        // Consume third time (should fail because expired_count is 0)
        let consumed3 = consume_file_code(&pool, fc.id, now).await.unwrap();
        assert!(consumed3.is_none());

        // Check expired filecodes
        let expired = get_expired_file_codes(&pool, now).await.unwrap();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].id, fc.id);

        // Count and sum size
        assert_eq!(count_total_files(&pool).await.unwrap(), 1);
        assert_eq!(sum_total_storage_size(&pool).await.unwrap(), 1024);

        // Delete
        assert!(delete_file_code(&pool, fc.id).await.unwrap());
        assert_eq!(count_total_files(&pool).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_keyvalue_crud() {
        let pool = init_pool(":memory:").await.unwrap();

        // Get non-existent
        let val = get_key_value(&pool, "settings").await.unwrap();
        assert!(val.is_none());

        // Set
        let kv = set_key_value(&pool, "settings", Some(r#"{"open_upload":true}"#)).await.unwrap();
        assert_eq!(kv.key, "settings");
        assert_eq!(kv.value, Some(r#"{"open_upload":true}"#.to_string()));

        // Update
        let kv2 = set_key_value(&pool, "settings", Some(r#"{"open_upload":false}"#)).await.unwrap();
        assert_eq!(kv2.value, Some(r#"{"open_upload":false}"#.to_string()));

        // Delete
        assert!(delete_key_value(&pool, "settings").await.unwrap());
        assert!(get_key_value(&pool, "settings").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_reservation_and_chunks() {
        let pool = init_pool(":memory:").await.unwrap();
        let now = Utc::now();

        // Reservation
        let res = insert_or_update_reservation(&pool, "tok1", 500, now + Duration::minutes(10)).await.unwrap();
        assert_eq!(res.size, 500);

        let active = get_active_reservation(&pool, "tok1", now).await.unwrap();
        assert!(active.is_some());
        assert_eq!(sum_active_reserved_size(&pool, now).await.unwrap(), 500);

        // Expired reservation purge
        let purged = delete_expired_reservations(&pool, now + Duration::minutes(15)).await.unwrap();
        assert_eq!(purged, 1);
        assert_eq!(sum_active_reserved_size(&pool, now).await.unwrap(), 0);

        // Upload chunk
        let chunk = UploadChunk {
            id: 0,
            upload_id: "u123".to_string(),
            chunk_index: 0,
            chunk_hash: "chash0".to_string(),
            total_chunks: 2,
            file_size: 200,
            chunk_size: 100,
            file_name: "test.bin".to_string(),
            save_path: Some("data/test.bin".to_string()),
            created_at: None,
            completed: true,
        };
        upsert_upload_chunk(&pool, &chunk).await.unwrap();

        let chunks = get_upload_chunks(&pool, "u123").await.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_hash, "chash0");

        let deleted_chunks = delete_upload_chunks(&pool, "u123").await.unwrap();
        assert_eq!(deleted_chunks, 1);
    }
}
