use chrono::{DateTime, Utc};
use fcb_core::AppError;
use serde_json::json;

use crate::models::{FileCode, KeyValue, PresignSession, StorageReservation};
use crate::pool::DbPool;

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
    pool: &DbPool,
    new_code: &NewFileCode,
) -> Result<FileCode, AppError> {
    match pool {
        DbPool::Sqlite(p) => {
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
            .fetch_one(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to insert file code (sqlite): {}", e)))?;

            Ok(row)
        }
        DbPool::Postgres(p) => {
            let row = sqlx::query_as::<_, FileCode>(
                r#"
                INSERT INTO filecodes (
                    code, prefix, suffix, uuid_file_name, file_path, size, text,
                    expired_at, expired_count, used_count, file_hash, is_chunked, upload_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
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
            .fetch_one(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to insert file code (pg): {}", e)))?;

            Ok(row)
        }
        DbPool::MySql(p) => {
            sqlx::query(
                r#"
                INSERT INTO filecodes (
                    code, prefix, suffix, uuid_file_name, file_path, size, text,
                    expired_at, expired_count, used_count, file_hash, is_chunked, upload_id
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
            .execute(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to insert file code (mysql): {}", e)))?;

            get_file_code_by_code(pool, &new_code.code)
                .await?
                .ok_or_else(|| AppError::Database("Inserted file code not found".to_string()))
        }
    }
}

/// Get a file code by its unique pickup code.
pub async fn get_file_code_by_code(
    pool: &DbPool,
    code: &str,
) -> Result<Option<FileCode>, AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query_as::<_, FileCode>(
                r#"
                SELECT id, code, prefix, suffix, uuid_file_name, file_path, size, text,
                       expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
                FROM filecodes
                WHERE code = ?
                "#,
            )
            .bind(code)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to fetch file code by code: {}", e)))
        }
        DbPool::Postgres(p) => {
            sqlx::query_as::<_, FileCode>(
                r#"
                SELECT id, code, prefix, suffix, uuid_file_name, file_path, size, text,
                       expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
                FROM filecodes
                WHERE code = $1
                "#,
            )
            .bind(code)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to fetch file code by code: {}", e)))
        }
        DbPool::MySql(p) => {
            sqlx::query_as::<_, FileCode>(
                r#"
                SELECT id, code, prefix, suffix, uuid_file_name, file_path, size, text,
                       expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
                FROM filecodes
                WHERE code = ?
                "#,
            )
            .bind(code)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to fetch file code by code: {}", e)))
        }
    }
}

/// Get a file code by its ID.
pub async fn get_file_code_by_id(
    pool: &DbPool,
    id: i64,
) -> Result<Option<FileCode>, AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query_as::<_, FileCode>(
                r#"
                SELECT id, code, prefix, suffix, uuid_file_name, file_path, size, text,
                       expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
                FROM filecodes
                WHERE id = ?
                "#,
            )
            .bind(id)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to fetch file code by id: {}", e)))
        }
        DbPool::Postgres(p) => {
            sqlx::query_as::<_, FileCode>(
                r#"
                SELECT id, code, prefix, suffix, uuid_file_name, file_path, size, text,
                       expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
                FROM filecodes
                WHERE id = $1
                "#,
            )
            .bind(id)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to fetch file code by id: {}", e)))
        }
        DbPool::MySql(p) => {
            sqlx::query_as::<_, FileCode>(
                r#"
                SELECT id, code, prefix, suffix, uuid_file_name, file_path, size, text,
                       expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
                FROM filecodes
                WHERE id = ?
                "#,
            )
            .bind(id)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to fetch file code by id: {}", e)))
        }
    }
}

/// Atomically consume/use a filecode if valid and not expired.
pub async fn consume_file_code(
    pool: &DbPool,
    id: i64,
    now: DateTime<Utc>,
) -> Result<Option<FileCode>, AppError> {
    match pool {
        DbPool::Sqlite(p) => {
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
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to consume file code: {}", e)))?;

            Ok(row)
        }
        DbPool::Postgres(p) => {
            let row = sqlx::query_as::<_, FileCode>(
                r#"
                UPDATE filecodes
                SET used_count = used_count + 1,
                    expired_count = CASE
                        WHEN expired_count > 0 THEN expired_count - 1
                        ELSE expired_count
                    END
                WHERE id = $1
                  AND (expired_count = -1 OR expired_count > 0)
                  AND (expired_at IS NULL OR expired_count > 0 OR expired_at >= $2)
                RETURNING id, code, prefix, suffix, uuid_file_name, file_path, size, text,
                          expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
                "#,
            )
            .bind(id)
            .bind(now)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to consume file code: {}", e)))?;

            Ok(row)
        }
        DbPool::MySql(p) => {
            let res = sqlx::query(
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
                "#,
            )
            .bind(id)
            .bind(now)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to consume file code: {}", e)))?;

            if res.rows_affected() > 0 {
                get_file_code_by_id(pool, id).await
            } else {
                Ok(None)
            }
        }
    }
}

/// Delete a file code by its ID. Returns true if a row was deleted.
pub async fn delete_file_code(pool: &DbPool, id: i64) -> Result<bool, AppError> {
    let rows_affected = match pool {
        DbPool::Sqlite(p) => sqlx::query("DELETE FROM filecodes WHERE id = ?")
            .bind(id)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::Postgres(p) => sqlx::query("DELETE FROM filecodes WHERE id = $1")
            .bind(id)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::MySql(p) => sqlx::query("DELETE FROM filecodes WHERE id = ?")
            .bind(id)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
    };
    Ok(rows_affected > 0)
}

/// Find all expired filecodes for background cleanup.
pub async fn get_expired_file_codes(
    pool: &DbPool,
    now: DateTime<Utc>,
) -> Result<Vec<FileCode>, AppError> {
    match pool {
        DbPool::Sqlite(p) => {
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
            .fetch_all(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to list expired file codes: {}", e)))
        }
        DbPool::Postgres(p) => {
            sqlx::query_as::<_, FileCode>(
                r#"
                SELECT id, code, prefix, suffix, uuid_file_name, file_path, size, text,
                       expired_at, expired_count, used_count, created_at, file_hash, is_chunked, upload_id
                FROM filecodes
                WHERE (expired_at IS NOT NULL AND expired_count < 0 AND expired_at < $1)
                   OR (expired_count = 0)
                "#,
            )
            .bind(now)
            .fetch_all(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to list expired file codes: {}", e)))
        }
        DbPool::MySql(p) => {
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
            .fetch_all(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to list expired file codes: {}", e)))
        }
    }
}

/// List all filecodes ordered by ID descending.
pub async fn list_all_files(pool: &DbPool) -> Result<Vec<FileCode>, AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query_as::<_, FileCode>("SELECT * FROM filecodes ORDER BY id DESC")
                .fetch_all(p)
                .await
                .map_err(|e| AppError::Database(e.to_string()))
        }
        DbPool::Postgres(p) => {
            sqlx::query_as::<_, FileCode>("SELECT * FROM filecodes ORDER BY id DESC")
                .fetch_all(p)
                .await
                .map_err(|e| AppError::Database(e.to_string()))
        }
        DbPool::MySql(p) => {
            sqlx::query_as::<_, FileCode>("SELECT * FROM filecodes ORDER BY id DESC")
                .fetch_all(p)
                .await
                .map_err(|e| AppError::Database(e.to_string()))
        }
    }
}

/// Count total stored filecodes.
pub async fn count_total_files(pool: &DbPool) -> Result<i64, AppError> {
    let row: (i64,) = match pool {
        DbPool::Sqlite(p) => sqlx::query_as("SELECT COUNT(*) FROM filecodes").fetch_one(p).await,
        DbPool::Postgres(p) => sqlx::query_as("SELECT COUNT(*) FROM filecodes").fetch_one(p).await,
        DbPool::MySql(p) => sqlx::query_as("SELECT COUNT(*) FROM filecodes").fetch_one(p).await,
    }
    .map_err(|e| AppError::Database(format!("Failed to count file codes: {}", e)))?;

    Ok(row.0)
}

/// Total used storage size of non-local-ref files.
pub async fn sum_total_storage_size(pool: &DbPool) -> Result<i64, AppError> {
    let row: (Option<i64>,) = match pool {
        DbPool::Sqlite(p) => {
            sqlx::query_as(
                "SELECT SUM(size) FROM filecodes WHERE file_path IS NULL OR file_path != 'local-ref'",
            )
            .fetch_one(p)
            .await
        }
        DbPool::Postgres(p) => {
            sqlx::query_as(
                "SELECT SUM(size) FROM filecodes WHERE file_path IS NULL OR file_path != 'local-ref'",
            )
            .fetch_one(p)
            .await
        }
        DbPool::MySql(p) => {
            sqlx::query_as(
                "SELECT SUM(size) FROM filecodes WHERE file_path IS NULL OR file_path != 'local-ref'",
            )
            .fetch_one(p)
            .await
        }
    }
    .map_err(|e| AppError::Database(format!("Failed to sum storage size: {}", e)))?;

    Ok(row.0.unwrap_or(0))
}

/// Total retrieval times.
pub async fn sum_total_used_count(pool: &DbPool) -> Result<i64, AppError> {
    let row: (Option<i64>,) = match pool {
        DbPool::Sqlite(p) => sqlx::query_as("SELECT SUM(used_count) FROM filecodes").fetch_one(p).await,
        DbPool::Postgres(p) => sqlx::query_as("SELECT SUM(used_count) FROM filecodes").fetch_one(p).await,
        DbPool::MySql(p) => sqlx::query_as("SELECT SUM(used_count) FROM filecodes").fetch_one(p).await,
    }
    .map_err(|e| AppError::Database(format!("Failed to sum used count: {}", e)))?;

    Ok(row.0.unwrap_or(0))
}

/// Update file expiration policy
pub async fn update_file_policy_expired_at(
    pool: &DbPool,
    id: i64,
    new_exp: Option<DateTime<Utc>>,
) -> Result<(), AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query("UPDATE filecodes SET expired_at = ? WHERE id = ?")
                .bind(new_exp)
                .bind(id)
                .execute(p)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }
        DbPool::Postgres(p) => {
            sqlx::query("UPDATE filecodes SET expired_at = $1 WHERE id = $2")
                .bind(new_exp)
                .bind(id)
                .execute(p)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }
        DbPool::MySql(p) => {
            sqlx::query("UPDATE filecodes SET expired_at = ? WHERE id = ?")
                .bind(new_exp)
                .bind(id)
                .execute(p)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }
    }
    Ok(())
}

pub async fn update_file_policy_make_permanent(pool: &DbPool, id: i64) -> Result<(), AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query("UPDATE filecodes SET expired_at = NULL, expired_count = -1 WHERE id = ?")
                .bind(id)
                .execute(p)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }
        DbPool::Postgres(p) => {
            sqlx::query("UPDATE filecodes SET expired_at = NULL, expired_count = -1 WHERE id = $1")
                .bind(id)
                .execute(p)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }
        DbPool::MySql(p) => {
            sqlx::query("UPDATE filecodes SET expired_at = NULL, expired_count = -1 WHERE id = ?")
                .bind(id)
                .execute(p)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }
    }
    Ok(())
}

pub async fn update_file_policy_download_limit(
    pool: &DbPool,
    id: i64,
    limit: i32,
) -> Result<(), AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query("UPDATE filecodes SET expired_count = ? WHERE id = ?")
                .bind(limit)
                .bind(id)
                .execute(p)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }
        DbPool::Postgres(p) => {
            sqlx::query("UPDATE filecodes SET expired_count = $1 WHERE id = $2")
                .bind(limit)
                .bind(id)
                .execute(p)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }
        DbPool::MySql(p) => {
            sqlx::query("UPDATE filecodes SET expired_count = ? WHERE id = ?")
                .bind(limit)
                .bind(id)
                .execute(p)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }
    }
    Ok(())
}

// ============================================================================
// KeyValue CRUD
// ============================================================================

/// Get KeyValue record by key.
pub async fn get_key_value(pool: &DbPool, key: &str) -> Result<Option<KeyValue>, AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query_as::<_, KeyValue>(
                "SELECT id, key, value, created_at FROM keyvalue WHERE key = ?",
            )
            .bind(key)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to get keyvalue: {}", e)))
        }
        DbPool::Postgres(p) => {
            sqlx::query_as::<_, KeyValue>(
                "SELECT id, key, value, created_at FROM keyvalue WHERE key = $1",
            )
            .bind(key)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to get keyvalue: {}", e)))
        }
        DbPool::MySql(p) => {
            sqlx::query_as::<_, KeyValue>(
                "SELECT id, `key`, `value`, created_at FROM keyvalue WHERE `key` = ?",
            )
            .bind(key)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to get keyvalue: {}", e)))
        }
    }
}

/// Set or update KeyValue record.
pub async fn set_key_value(
    pool: &DbPool,
    key: &str,
    value: &str,
) -> Result<KeyValue, AppError> {
    match pool {
        DbPool::Sqlite(p) => {
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
            .fetch_one(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to set keyvalue (sqlite): {}", e)))?;
            Ok(row)
        }
        DbPool::Postgres(p) => {
            let row = sqlx::query_as::<_, KeyValue>(
                r#"
                INSERT INTO keyvalue (key, value)
                VALUES ($1, $2)
                ON CONFLICT(key) DO UPDATE SET value = EXCLUDED.value
                RETURNING id, key, value, created_at
                "#,
            )
            .bind(key)
            .bind(value)
            .fetch_one(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to set keyvalue (pg): {}", e)))?;
            Ok(row)
        }
        DbPool::MySql(p) => {
            sqlx::query(
                r#"
                INSERT INTO keyvalue (`key`, `value`)
                VALUES (?, ?)
                ON DUPLICATE KEY UPDATE `value` = VALUES(`value`)
                "#,
            )
            .bind(key)
            .bind(value)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to set keyvalue (mysql): {}", e)))?;

            get_key_value(pool, key)
                .await?
                .ok_or_else(|| AppError::Database("Keyvalue inserted not found".to_string()))
        }
    }
}

/// Delete KeyValue record.
pub async fn delete_key_value(pool: &DbPool, key: &str) -> Result<bool, AppError> {
    let rows_affected = match pool {
        DbPool::Sqlite(p) => sqlx::query("DELETE FROM keyvalue WHERE key = ?")
            .bind(key)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::Postgres(p) => sqlx::query("DELETE FROM keyvalue WHERE key = $1")
            .bind(key)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::MySql(p) => sqlx::query("DELETE FROM keyvalue WHERE `key` = ?")
            .bind(key)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
    };
    Ok(rows_affected > 0)
}

// ============================================================================
// StorageReservation CRUD
// ============================================================================

/// Insert or update a storage reservation record.
pub async fn insert_or_update_reservation(
    pool: &DbPool,
    token: &str,
    size: i64,
    expires_at: DateTime<Utc>,
) -> Result<StorageReservation, AppError> {
    match pool {
        DbPool::Sqlite(p) => {
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
            .fetch_one(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to upsert reservation: {}", e)))?;
            Ok(row)
        }
        DbPool::Postgres(p) => {
            let row = sqlx::query_as::<_, StorageReservation>(
                r#"
                INSERT INTO storagereservation (token, size, expires_at)
                VALUES ($1, $2, $3)
                ON CONFLICT(token) DO UPDATE SET size = EXCLUDED.size, expires_at = EXCLUDED.expires_at
                RETURNING id, token, size, expires_at
                "#,
            )
            .bind(token)
            .bind(size)
            .bind(expires_at)
            .fetch_one(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to upsert reservation: {}", e)))?;
            Ok(row)
        }
        DbPool::MySql(p) => {
            sqlx::query(
                r#"
                INSERT INTO storagereservation (token, size, expires_at)
                VALUES (?, ?, ?)
                ON DUPLICATE KEY UPDATE size = VALUES(size), expires_at = VALUES(expires_at)
                "#,
            )
            .bind(token)
            .bind(size)
            .bind(expires_at)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to upsert reservation: {}", e)))?;

            get_active_reservation(pool, token, Utc::now() - chrono::Duration::hours(24))
                .await?
                .ok_or_else(|| AppError::Database("Reservation not found after upsert".to_string()))
        }
    }
}

/// Get active reservation by token if not expired.
pub async fn get_active_reservation(
    pool: &DbPool,
    token: &str,
    now: DateTime<Utc>,
) -> Result<Option<StorageReservation>, AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query_as::<_, StorageReservation>(
                "SELECT id, token, size, expires_at FROM storagereservation WHERE token = ? AND expires_at > ?",
            )
            .bind(token)
            .bind(now)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to get reservation: {}", e)))
        }
        DbPool::Postgres(p) => {
            sqlx::query_as::<_, StorageReservation>(
                "SELECT id, token, size, expires_at FROM storagereservation WHERE token = $1 AND expires_at > $2",
            )
            .bind(token)
            .bind(now)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to get reservation: {}", e)))
        }
        DbPool::MySql(p) => {
            sqlx::query_as::<_, StorageReservation>(
                "SELECT id, token, size, expires_at FROM storagereservation WHERE token = ? AND expires_at > ?",
            )
            .bind(token)
            .bind(now)
            .fetch_optional(p)
            .await
            .map_err(|e| AppError::Database(format!("Failed to get reservation: {}", e)))
        }
    }
}

/// Delete reservation by token.
pub async fn delete_reservation(pool: &DbPool, token: &str) -> Result<bool, AppError> {
    let rows_affected = match pool {
        DbPool::Sqlite(p) => sqlx::query("DELETE FROM storagereservation WHERE token = ?")
            .bind(token)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::Postgres(p) => sqlx::query("DELETE FROM storagereservation WHERE token = $1")
            .bind(token)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::MySql(p) => sqlx::query("DELETE FROM storagereservation WHERE token = ?")
            .bind(token)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
    };

    Ok(rows_affected > 0)
}

/// Delete expired reservations.
pub async fn delete_expired_reservations(pool: &DbPool, now: DateTime<Utc>) -> Result<u64, AppError> {
    let rows_affected = match pool {
        DbPool::Sqlite(p) => sqlx::query("DELETE FROM storagereservation WHERE expires_at < ?")
            .bind(now)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::Postgres(p) => sqlx::query("DELETE FROM storagereservation WHERE expires_at < $1")
            .bind(now)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::MySql(p) => sqlx::query("DELETE FROM storagereservation WHERE expires_at < ?")
            .bind(now)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
    };

    Ok(rows_affected)
}

/// Sum total active reserved size.
pub async fn sum_active_reserved_size(pool: &DbPool, now: DateTime<Utc>) -> Result<i64, AppError> {
    let row: (Option<i64>,) = match pool {
        DbPool::Sqlite(p) => {
            sqlx::query_as(
                "SELECT SUM(size) FROM storagereservation WHERE expires_at > ?",
            )
            .bind(now)
            .fetch_one(p)
            .await
        }
        DbPool::Postgres(p) => {
            sqlx::query_as(
                "SELECT SUM(size) FROM storagereservation WHERE expires_at > $1",
            )
            .bind(now)
            .fetch_one(p)
            .await
        }
        DbPool::MySql(p) => {
            sqlx::query_as(
                "SELECT SUM(size) FROM storagereservation WHERE expires_at > ?",
            )
            .bind(now)
            .fetch_one(p)
            .await
        }
    }
    .map_err(|e| AppError::Database(format!("Failed to sum reserved size: {}", e)))?;

    Ok(row.0.unwrap_or(0))
}

// ============================================================================
// UploadChunk CRUD
// ============================================================================

pub async fn insert_chunk_session(
    pool: &DbPool,
    upload_id: &str,
    file_name: &str,
    file_size: i64,
    chunk_size: i32,
    total_chunks: i32,
    file_hash: &str,
) -> Result<(), AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query(
                r#"
                INSERT INTO uploadchunk (
                    upload_id, chunk_index, chunk_hash, total_chunks, file_size, chunk_size, file_name, completed
                ) VALUES (?, -1, ?, ?, ?, ?, ?, 0)
                "#,
            )
            .bind(upload_id)
            .bind(file_hash)
            .bind(total_chunks)
            .bind(file_size)
            .bind(chunk_size)
            .bind(file_name)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
        DbPool::Postgres(p) => {
            sqlx::query(
                r#"
                INSERT INTO uploadchunk (
                    upload_id, chunk_index, chunk_hash, total_chunks, file_size, chunk_size, file_name, completed
                ) VALUES ($1, -1, $2, $3, $4, $5, $6, FALSE)
                "#,
            )
            .bind(upload_id)
            .bind(file_hash)
            .bind(total_chunks)
            .bind(file_size)
            .bind(chunk_size)
            .bind(file_name)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
        DbPool::MySql(p) => {
            sqlx::query(
                r#"
                INSERT INTO uploadchunk (
                    upload_id, chunk_index, chunk_hash, total_chunks, file_size, chunk_size, file_name, completed
                ) VALUES (?, -1, ?, ?, ?, ?, ?, 0)
                "#,
            )
            .bind(upload_id)
            .bind(file_hash)
            .bind(total_chunks)
            .bind(file_size)
            .bind(chunk_size)
            .bind(file_name)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
    }
    Ok(())
}

pub async fn get_chunk_session(
    pool: &DbPool,
    upload_id: &str,
) -> Result<Option<(String, i64, i32, i32)>, AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query_as(
                "SELECT file_name, file_size, chunk_size, total_chunks FROM uploadchunk WHERE upload_id = ? AND chunk_index = -1",
            )
            .bind(upload_id)
            .fetch_optional(p)
            .await
        }
        DbPool::Postgres(p) => {
            sqlx::query_as(
                "SELECT file_name, file_size, chunk_size, total_chunks FROM uploadchunk WHERE upload_id = $1 AND chunk_index = -1",
            )
            .bind(upload_id)
            .fetch_optional(p)
            .await
        }
        DbPool::MySql(p) => {
            sqlx::query_as(
                "SELECT file_name, file_size, chunk_size, total_chunks FROM uploadchunk WHERE upload_id = ? AND chunk_index = -1",
            )
            .bind(upload_id)
            .fetch_optional(p)
            .await
        }
    }
    .map_err(|e| AppError::Database(e.to_string()))
}

pub async fn mark_chunk_completed(
    pool: &DbPool,
    upload_id: &str,
    chunk_index: i32,
) -> Result<(), AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query(
                "INSERT OR REPLACE INTO uploadchunk (upload_id, chunk_index, chunk_hash, total_chunks, file_size, chunk_size, file_name, completed) VALUES (?, ?, '', 0, 0, 0, '', 1)"
            )
            .bind(upload_id)
            .bind(chunk_index)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
        DbPool::Postgres(p) => {
            sqlx::query(
                "INSERT INTO uploadchunk (upload_id, chunk_index, chunk_hash, total_chunks, file_size, chunk_size, file_name, completed) VALUES ($1, $2, '', 0, 0, 0, '', TRUE)"
            )
            .bind(upload_id)
            .bind(chunk_index)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
        DbPool::MySql(p) => {
            sqlx::query(
                "INSERT INTO uploadchunk (upload_id, chunk_index, chunk_hash, total_chunks, file_size, chunk_size, file_name, completed) VALUES (?, ?, '', 0, 0, 0, '', 1)"
            )
            .bind(upload_id)
            .bind(chunk_index)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
    }
    Ok(())
}

pub async fn get_uploaded_chunk_indexes(
    pool: &DbPool,
    upload_id: &str,
) -> Result<Vec<i32>, AppError> {
    let rows: Vec<(i32,)> = match pool {
        DbPool::Sqlite(p) => {
            sqlx::query_as("SELECT chunk_index FROM uploadchunk WHERE upload_id = ? AND completed = 1")
                .bind(upload_id)
                .fetch_all(p)
                .await
        }
        DbPool::Postgres(p) => {
            sqlx::query_as("SELECT chunk_index FROM uploadchunk WHERE upload_id = $1 AND completed = TRUE")
                .bind(upload_id)
                .fetch_all(p)
                .await
        }
        DbPool::MySql(p) => {
            sqlx::query_as("SELECT chunk_index FROM uploadchunk WHERE upload_id = ? AND completed = 1")
                .bind(upload_id)
                .fetch_all(p)
                .await
        }
    }
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(rows.into_iter().map(|(idx,)| idx).collect())
}

pub async fn delete_upload_chunks(pool: &DbPool, upload_id: &str) -> Result<u64, AppError> {
    let rows_affected = match pool {
        DbPool::Sqlite(p) => sqlx::query("DELETE FROM uploadchunk WHERE upload_id = ?")
            .bind(upload_id)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::Postgres(p) => sqlx::query("DELETE FROM uploadchunk WHERE upload_id = $1")
            .bind(upload_id)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::MySql(p) => sqlx::query("DELETE FROM uploadchunk WHERE upload_id = ?")
            .bind(upload_id)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
    };

    Ok(rows_affected)
}

// ============================================================================
// PresignSession CRUD
// ============================================================================

pub async fn insert_presign_session(
    pool: &DbPool,
    upload_id: &str,
    file_name: &str,
    file_size: i64,
    save_path: &str,
    mode: &str,
    expire_value: i32,
    expire_style: &str,
    expires_at: DateTime<Utc>,
) -> Result<(), AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query(
                r#"
                INSERT INTO presignuploadsession (
                    upload_id, file_name, file_size, save_path, mode, expire_value, expire_style, expires_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(upload_id)
            .bind(file_name)
            .bind(file_size)
            .bind(save_path)
            .bind(mode)
            .bind(expire_value)
            .bind(expire_style)
            .bind(expires_at)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
        DbPool::Postgres(p) => {
            sqlx::query(
                r#"
                INSERT INTO presignuploadsession (
                    upload_id, file_name, file_size, save_path, mode, expire_value, expire_style, expires_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(upload_id)
            .bind(file_name)
            .bind(file_size)
            .bind(save_path)
            .bind(mode)
            .bind(expire_value)
            .bind(expire_style)
            .bind(expires_at)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
        DbPool::MySql(p) => {
            sqlx::query(
                r#"
                INSERT INTO presignuploadsession (
                    upload_id, file_name, file_size, save_path, mode, expire_value, expire_style, expires_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(upload_id)
            .bind(file_name)
            .bind(file_size)
            .bind(save_path)
            .bind(mode)
            .bind(expire_value)
            .bind(expire_style)
            .bind(expires_at)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
    }
    Ok(())
}

pub async fn get_presign_session(
    pool: &DbPool,
    upload_id: &str,
) -> Result<Option<PresignSession>, AppError> {
    match pool {
        DbPool::Sqlite(p) => {
            sqlx::query_as::<_, PresignSession>(
                "SELECT * FROM presignuploadsession WHERE upload_id = ?",
            )
            .bind(upload_id)
            .fetch_optional(p)
            .await
        }
        DbPool::Postgres(p) => {
            sqlx::query_as::<_, PresignSession>(
                "SELECT * FROM presignuploadsession WHERE upload_id = $1",
            )
            .bind(upload_id)
            .fetch_optional(p)
            .await
        }
        DbPool::MySql(p) => {
            sqlx::query_as::<_, PresignSession>(
                "SELECT * FROM presignuploadsession WHERE upload_id = ?",
            )
            .bind(upload_id)
            .fetch_optional(p)
            .await
        }
    }
    .map_err(|e| AppError::Database(e.to_string()))
}

pub async fn delete_presign_session(pool: &DbPool, upload_id: &str) -> Result<bool, AppError> {
    let rows_affected = match pool {
        DbPool::Sqlite(p) => sqlx::query("DELETE FROM presignuploadsession WHERE upload_id = ?")
            .bind(upload_id)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::Postgres(p) => sqlx::query("DELETE FROM presignuploadsession WHERE upload_id = $1")
            .bind(upload_id)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::MySql(p) => sqlx::query("DELETE FROM presignuploadsession WHERE upload_id = ?")
            .bind(upload_id)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
    };

    Ok(rows_affected > 0)
}

pub async fn delete_expired_presign_sessions(
    pool: &DbPool,
    now: DateTime<Utc>,
) -> Result<u64, AppError> {
    let rows_affected = match pool {
        DbPool::Sqlite(p) => sqlx::query("DELETE FROM presignuploadsession WHERE expires_at < ?")
            .bind(now)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::Postgres(p) => sqlx::query("DELETE FROM presignuploadsession WHERE expires_at < $1")
            .bind(now)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
        DbPool::MySql(p) => sqlx::query("DELETE FROM presignuploadsession WHERE expires_at < ?")
            .bind(now)
            .execute(p)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .rows_affected(),
    };

    Ok(rows_affected)
}

// ============================================================================
// View Presets
// ============================================================================

pub async fn get_view_presets(pool: &DbPool) -> Result<Vec<serde_json::Value>, AppError> {
    if let Some(kv) = get_key_value(pool, "file_view_presets").await? {
        if let Some(val_str) = kv.value {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&val_str) {
                if val.is_array() {
                    return Ok(val.as_array().cloned().unwrap_or_default());
                } else if let Some(presets) = val.get("presets").and_then(|v| v.as_array()) {
                    return Ok(presets.clone());
                }
            }
        }
    }
    Ok(Vec::new())
}

pub async fn save_view_preset(
    pool: &DbPool,
    id: Option<String>,
    name: String,
    filters: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    let mut presets = get_view_presets(pool).await?;
    let now_str = Utc::now().to_rfc3339();
    let preset_id = id.unwrap_or_else(|| format!("vp_{}", uuid::Uuid::new_v4().simple()));

    let mut found = false;
    let mut updated_item = json!({});

    for item in &mut presets {
        if item.get("id").and_then(|v| v.as_str()) == Some(&preset_id) {
            if let Some(obj) = item.as_object_mut() {
                obj.insert("name".to_string(), json!(name));
                obj.insert("filters".to_string(), filters.clone());
                obj.insert("updated_at".to_string(), json!(now_str));
            }
            updated_item = item.clone();
            found = true;
            break;
        }
    }

    if !found {
        updated_item = json!({
            "id": preset_id,
            "name": name,
            "filters": filters,
            "created_at": now_str,
            "updated_at": now_str
        });
        presets.push(updated_item.clone());
    }

    let json_str = serde_json::to_string(&presets).map_err(|e| AppError::Internal(e.to_string()))?;
    set_key_value(pool, "file_view_presets", &json_str).await?;

    Ok(updated_item)
}

pub async fn delete_view_preset(pool: &DbPool, preset_id: &str) -> Result<bool, AppError> {
    let mut presets = get_view_presets(pool).await?;
    let before_len = presets.len();
    presets.retain(|item| item.get("id").and_then(|v| v.as_str()) != Some(preset_id));

    if presets.len() != before_len {
        let json_str = serde_json::to_string(&presets).map_err(|e| AppError::Internal(e.to_string()))?;
        set_key_value(pool, "file_view_presets", &json_str).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_pool;

    #[tokio::test]
    async fn test_filecode_crud_and_consumption() {
        let pool = init_pool(":memory:").await.unwrap();

        let new_code = NewFileCode {
            code: "TEST1".to_string(),
            prefix: "2026/01/01".to_string(),
            suffix: "test.png".to_string(),
            uuid_file_name: Some("test.png".to_string()),
            file_path: Some("2026/01/01".to_string()),
            size: 1024,
            text: None,
            expired_at: Some(Utc::now() + chrono::Duration::hours(24)),
            expired_count: 5,
            used_count: 0,
            file_hash: None,
            is_chunked: false,
            upload_id: None,
        };

        let inserted = insert_file_code(&pool, &new_code).await.unwrap();
        assert_eq!(inserted.code, "TEST1");
        assert_eq!(inserted.expired_count, 5);

        let fetched = get_file_code_by_code(&pool, "TEST1").await.unwrap().unwrap();
        assert_eq!(fetched.id, inserted.id);

        let consumed = consume_file_code(&pool, inserted.id, Utc::now())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(consumed.used_count, 1);
        assert_eq!(consumed.expired_count, 4);

        let deleted = delete_file_code(&pool, inserted.id).await.unwrap();
        assert!(deleted);

        let missing = get_file_code_by_code(&pool, "TEST1").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_keyvalue_crud() {
        let pool = init_pool(":memory:").await.unwrap();

        set_key_value(&pool, "site_name", "My FCB").await.unwrap();
        let val = get_key_value(&pool, "site_name").await.unwrap().unwrap();
        assert_eq!(val.value.as_deref(), Some("My FCB"));

        set_key_value(&pool, "site_name", "Updated FCB").await.unwrap();
        let updated = get_key_value(&pool, "site_name").await.unwrap().unwrap();
        assert_eq!(updated.value.as_deref(), Some("Updated FCB"));

        let deleted = delete_key_value(&pool, "site_name").await.unwrap();
        assert!(deleted);
    }
}
