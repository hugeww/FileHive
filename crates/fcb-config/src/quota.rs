use chrono::Utc;
use fcb_core::AppError;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// 存储使用统计
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageUsage {
    pub total_bytes: i64,
    pub limit_bytes: i64,
    pub used_bytes: i64,
}

impl StorageUsage {
    /// 预留占用的字节数（未完成上传的预留容量）
    pub fn reserved_bytes(&self) -> i64 {
        self.total_bytes.saturating_sub(self.used_bytes)
    }

    /// 剩余可用字节数（若未配置限制则为 None）
    pub fn available_bytes(&self) -> Option<i64> {
        if self.limit_bytes > 0 {
            Some((self.limit_bytes - self.total_bytes).max(0))
        } else {
            None
        }
    }

    /// 已使用比例 0.0 - 1.0（若未配置限制则为 0.0）
    pub fn used_ratio(&self) -> f64 {
        if self.limit_bytes > 0 {
            (self.total_bytes as f64 / self.limit_bytes as f64).min(1.0)
        } else {
            0.0
        }
    }
}

/// 获取当前存储使用情况
/// 聚合计算 filecodes 表（非 local-ref）实际占用与 storagereservation 表未过期预留
pub async fn get_storage_usage(pool: &SqlitePool, limit: i64) -> Result<StorageUsage, AppError> {
    let now = Utc::now();
    let used_bytes: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(size), 0) FROM filecodes WHERE file_path IS NULL OR file_path != 'local-ref'"
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let reserved_bytes: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(size), 0) FROM storagereservation WHERE expires_at > ?"
    )
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let total_bytes = used_bytes + reserved_bytes;

    Ok(StorageUsage {
        total_bytes,
        limit_bytes: limit.max(0),
        used_bytes,
    })
}

/// 上传容量原子预留（带指定容量上限 limit）
/// 遵循 Python 版 apps/base/quota.py 语义：
/// 1. 若 limit <= 0 或 size <= 0 则不限制；
/// 2. 幂等性：同一 token 且 size 一致直接通过；不同 size 报 409；
/// 3. 原子 SQL 条件插入，超出限额报 507。
pub async fn reserve_storage_with_limit(
    pool: &SqlitePool,
    token: &str,
    size: i64,
    ttl_secs: i64,
    limit: i64,
) -> Result<(), AppError> {
    let requested_size = size.max(0);
    if limit <= 0 || requested_size == 0 {
        return Ok(());
    }

    let now = Utc::now();
    let expires_at = now + chrono::Duration::seconds(ttl_secs);

    // 1. 清理该 token 已过期的历史记录
    sqlx::query("DELETE FROM storagereservation WHERE token = ? AND expires_at <= ?")
        .bind(token)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    // 2. 检查现有未过期预留（幂等与一致性校验）
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT size FROM storagereservation WHERE token = ? AND expires_at > ?"
    )
    .bind(token)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    if let Some((existing_size,)) = existing {
        if existing_size == requested_size {
            return Ok(());
        }
        return Err(AppError::StorageError {
            status_code: 409,
            detail: "上传容量预留信息不一致".to_string(),
        });
    }

    // 3. 原子 SQL 插入：仅在（已用 + 预留 + 请求）<= limit 时成功
    let sql = r#"
        INSERT INTO storagereservation (token, size, expires_at)
        SELECT ?1, ?2, ?3
        WHERE (
            COALESCE((SELECT SUM(size) FROM filecodes WHERE file_path IS NULL OR file_path != 'local-ref'), 0)
            + COALESCE((SELECT SUM(size) FROM storagereservation WHERE expires_at > ?4), 0)
            + ?5
        ) <= ?6
    "#;

    let result = sqlx::query(sql)
        .bind(token)
        .bind(requested_size)
        .bind(expires_at)
        .bind(now)
        .bind(requested_size)
        .bind(limit)
        .execute(pool)
        .await;

    match result {
        Ok(res) => {
            if res.rows_affected() == 1 {
                return Ok(());
            }
        }
        Err(_) => {
            // 并发冲突时核验是否存在合法记录
            let concurrent: Option<(i64,)> = sqlx::query_as(
                "SELECT size FROM storagereservation WHERE token = ? AND size = ? AND expires_at > ?"
            )
            .bind(token)
            .bind(requested_size)
            .bind(now)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

            if concurrent.is_some() {
                return Ok(());
            }
        }
    }

    // 再次核验并发写入
    let concurrent: Option<(i64,)> = sqlx::query_as(
        "SELECT size FROM storagereservation WHERE token = ? AND size = ? AND expires_at > ?"
    )
    .bind(token)
    .bind(requested_size)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    if concurrent.is_some() {
        return Ok(());
    }

    Err(AppError::StorageError {
        status_code: 507,
        detail: "存储空间已达到管理员设置的容量上限".to_string(),
    })
}

/// 上传容量原子预留（从 keyvalue 表读取 storage_limit 配置）
pub async fn reserve_storage(
    pool: &SqlitePool,
    token: &str,
    size: i64,
    ttl_secs: i64,
) -> Result<(), AppError> {
    let limit_row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT value FROM keyvalue WHERE key = 'settings'"
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let limit = match limit_row {
        Some((Some(json_str),)) => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                val.get("storage_limit")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
            } else {
                0
            }
        }
        _ => 0,
    };

    reserve_storage_with_limit(pool, token, size, ttl_secs, limit).await
}

/// 释放预留容量
pub async fn release_storage(pool: &SqlitePool, token: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM storagereservation WHERE token = ?")
        .bind(token)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE filecodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                code VARCHAR(255) NOT NULL UNIQUE,
                file_path VARCHAR(255),
                size BIGINT NOT NULL DEFAULT 0
            );",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE storagereservation (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                token VARCHAR(64) NOT NULL UNIQUE,
                size BIGINT NOT NULL,
                expires_at TIMESTAMP NOT NULL
            );",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE keyvalue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key VARCHAR(255) NOT NULL UNIQUE,
                value JSON
            );",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    #[test]
    fn test_storage_usage_methods() {
        let usage = StorageUsage {
            total_bytes: 800,
            limit_bytes: 1000,
            used_bytes: 500,
        };
        assert_eq!(usage.reserved_bytes(), 300);
        assert_eq!(usage.available_bytes(), Some(200));
        assert!((usage.used_ratio() - 0.8).abs() < 1e-6);

        let unlim = StorageUsage {
            total_bytes: 800,
            limit_bytes: 0,
            used_bytes: 500,
        };
        assert_eq!(unlim.available_bytes(), None);
        assert_eq!(unlim.used_ratio(), 0.0);
    }

    #[tokio::test]
    async fn test_quota_workflow() {
        let pool = setup_test_db().await;

        // 插入已存在的文件：100 字节正常文件，200 字节 local-ref 文件（应被排除）
        sqlx::query("INSERT INTO filecodes (code, file_path, size) VALUES ('code1', 'data/path', 100)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO filecodes (code, file_path, size) VALUES ('code2', 'local-ref', 200)")
            .execute(&pool)
            .await
            .unwrap();

        let usage = get_storage_usage(&pool, 1000).await.unwrap();
        assert_eq!(usage.used_bytes, 100);
        assert_eq!(usage.total_bytes, 100);
        assert_eq!(usage.limit_bytes, 1000);

        // 预留 500 字节，限额 1000 字节
        reserve_storage_with_limit(&pool, "token-1", 500, 3600, 1000).await.unwrap();

        let usage2 = get_storage_usage(&pool, 1000).await.unwrap();
        assert_eq!(usage2.used_bytes, 100);
        assert_eq!(usage2.reserved_bytes(), 500);
        assert_eq!(usage2.total_bytes, 600);

        // 幂等重试同 token 同 size
        reserve_storage_with_limit(&pool, "token-1", 500, 3600, 1000).await.unwrap();

        // 同 token 不同 size 应返回 409
        let err_409 = reserve_storage_with_limit(&pool, "token-1", 600, 3600, 1000)
            .await
            .unwrap_err();
        assert_eq!(err_409.status_code(), 409);

        // 尝试预留 500 字节，当前 100已用 + 500预留 + 500请求 = 1100 > 1000，应返回 507
        let err_507 = reserve_storage_with_limit(&pool, "token-2", 500, 3600, 1000)
            .await
            .unwrap_err();
        assert_eq!(err_507.status_code(), 507);

        // 释放 token-1
        release_storage(&pool, "token-1").await.unwrap();

        let usage3 = get_storage_usage(&pool, 1000).await.unwrap();
        assert_eq!(usage3.used_bytes, 100);
        assert_eq!(usage3.reserved_bytes(), 0);
        assert_eq!(usage3.total_bytes, 100);

        // 释放后再预留 token-2 (500 字节) 应成功
        reserve_storage_with_limit(&pool, "token-2", 500, 3600, 1000).await.unwrap();
    }

    #[tokio::test]
    async fn test_reserve_storage_reads_from_keyvalue() {
        let pool = setup_test_db().await;

        // 设置 storage_limit 为 500
        sqlx::query("INSERT INTO keyvalue (key, value) VALUES ('settings', '{\"storage_limit\": 500}')")
            .execute(&pool)
            .await
            .unwrap();

        // 预留 300 字节 -> 成功
        reserve_storage(&pool, "tok-a", 300, 3600).await.unwrap();

        // 预留 300 字节 -> 超出 500 限制，报 507
        let err = reserve_storage(&pool, "tok-b", 300, 3600).await.unwrap_err();
        assert_eq!(err.status_code(), 507);
    }
}
