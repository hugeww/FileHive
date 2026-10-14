use chrono::Utc;
use filehive_core::AppError;
use filehive_db::crud::{
    delete_reservation, get_active_reservation, get_key_value, insert_or_update_reservation,
    sum_active_reserved_size, sum_total_storage_size,
};
use filehive_db::DbPool;
use serde::{Deserialize, Serialize};

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
pub async fn get_storage_usage(pool: &DbPool, limit: i64) -> Result<StorageUsage, AppError> {
    let now = Utc::now();
    let used_bytes = sum_total_storage_size(pool).await?;
    let reserved_bytes = sum_active_reserved_size(pool, now).await?;
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
/// 3. 超出限额报 507。
pub async fn reserve_storage_with_limit(
    pool: &DbPool,
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

    // 检查现有未过期预留（幂等与一致性校验）
    if let Some(existing) = get_active_reservation(pool, token, now).await? {
        if existing.size == requested_size {
            return Ok(());
        }
        return Err(AppError::StorageError {
            status_code: 409,
            detail: "上传容量预留信息不一致".to_string(),
        });
    }

    // 检查是否超出限额
    let usage = get_storage_usage(pool, limit).await?;
    if usage.total_bytes + requested_size > limit {
        return Err(AppError::StorageError {
            status_code: 507,
            detail: "存储空间已达到管理员设置的容量上限".to_string(),
        });
    }

    // 写入预留
    insert_or_update_reservation(pool, token, requested_size, expires_at).await?;

    Ok(())
}

/// 上传容量原子预留（从 keyvalue 表读取 storage_limit 配置）
pub async fn reserve_storage(
    pool: &DbPool,
    token: &str,
    size: i64,
    ttl_secs: i64,
) -> Result<(), AppError> {
    let limit = if let Some(kv) = get_key_value(pool, "settings").await? {
        if let Some(json_str) = kv.value {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                val.get("storage_limit").and_then(|v| v.as_i64()).unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    };

    reserve_storage_with_limit(pool, token, size, ttl_secs, limit).await
}

/// 释放预留容量
pub async fn release_storage(pool: &DbPool, token: &str) -> Result<(), AppError> {
    delete_reservation(pool, token).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> DbPool {
        filehive_db::init_pool(":memory:").await.unwrap()
    }

    #[tokio::test]
    async fn test_quota_workflow() {
        let pool = setup_test_db().await;

        let usage = get_storage_usage(&pool, 1000).await.unwrap();
        assert_eq!(usage.total_bytes, 0);

        reserve_storage_with_limit(&pool, "t1", 500, 60, 1000)
            .await
            .unwrap();
        let usage = get_storage_usage(&pool, 1000).await.unwrap();
        assert_eq!(usage.total_bytes, 500);

        // 超额应该失败
        let err = reserve_storage_with_limit(&pool, "t2", 600, 60, 1000)
            .await
            .unwrap_err();
        assert_eq!(err.status_code(), 507);

        release_storage(&pool, "t1").await.unwrap();
        let usage = get_storage_usage(&pool, 1000).await.unwrap();
        assert_eq!(usage.total_bytes, 0);
    }
}
