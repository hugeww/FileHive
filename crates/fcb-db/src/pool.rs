use fcb_core::AppError;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{MySqlPool, PgPool, SqlitePool};
use std::str::FromStr;

/// 数据库类型枚举
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DbKind {
    Sqlite,
    Postgres,
    MySql,
}

/// 统一数据库连接池枚举
#[derive(Clone, Debug)]
pub enum DbPool {
    Sqlite(SqlitePool),
    Postgres(PgPool),
    MySql(MySqlPool),
}

impl DbPool {
    pub fn kind(&self) -> DbKind {
        match self {
            DbPool::Sqlite(_) => DbKind::Sqlite,
            DbPool::Postgres(_) => DbKind::Postgres,
            DbPool::MySql(_) => DbKind::MySql,
        }
    }

    pub fn as_sqlite(&self) -> Option<&SqlitePool> {
        match self {
            DbPool::Sqlite(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_postgres(&self) -> Option<&PgPool> {
        match self {
            DbPool::Postgres(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_mysql(&self) -> Option<&MySqlPool> {
        match self {
            DbPool::MySql(p) => Some(p),
            _ => None,
        }
    }
}

/// 根据数据库 URL 检测数据库类型
pub fn detect_db_kind(url: &str) -> DbKind {
    let lower = url.trim().to_lowercase();
    if lower.starts_with("postgres://") || lower.starts_with("postgresql://") {
        DbKind::Postgres
    } else if lower.starts_with("mysql://") || lower.starts_with("mariadb://") {
        DbKind::MySql
    } else {
        DbKind::Sqlite
    }
}

/// 初始化数据库连接池并自动执行建表与迁移
pub async fn init_pool(db_url: &str) -> Result<DbPool, AppError> {
    let kind = detect_db_kind(db_url);

    let pool = match kind {
        DbKind::Postgres => {
            let p = PgPoolOptions::new()
                .max_connections(20)
                .connect(db_url)
                .await
                .map_err(|e| AppError::Database(format!("Failed to connect to PostgreSQL: {}", e)))?;
            DbPool::Postgres(p)
        }
        DbKind::MySql => {
            let p = MySqlPoolOptions::new()
                .max_connections(20)
                .connect(db_url)
                .await
                .map_err(|e| AppError::Database(format!("Failed to connect to MySQL: {}", e)))?;
            DbPool::MySql(p)
        }
        DbKind::Sqlite => {
            // 如果是常规文件路径且非 :memory:，自动创建父级目录
            let clean_path = db_url.strip_prefix("sqlite:").unwrap_or(db_url);
            if clean_path != ":memory:" && !clean_path.starts_with("sqlite::memory:") {
                if let Some(parent) = std::path::Path::new(clean_path).parent() {
                    if !parent.as_os_str().is_empty() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                }
            }

            let connect_opts = SqliteConnectOptions::from_str(db_url)
                .map_err(|e| {
                    AppError::Database(format!("Invalid SQLite connection path '{}': {}", db_url, e))
                })?
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
                .busy_timeout(std::time::Duration::from_millis(10000))
                .foreign_keys(true);

            let p = SqlitePoolOptions::new()
                .max_connections(20)
                .connect_with(connect_opts)
                .await
                .map_err(|e| AppError::Database(format!("Failed to connect to SQLite: {}", e)))?;
            DbPool::Sqlite(p)
        }
    };

    // 自动运行迁移/建表
    crate::migrations::run_migrations(&pool).await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_sqlite_in_memory() {
        let pool = init_pool(":memory:")
            .await
            .expect("Failed to init in-memory sqlite pool");
        assert_eq!(pool.kind(), DbKind::Sqlite);
    }

    #[test]
    fn test_detect_db_kind() {
        assert_eq!(detect_db_kind("postgres://user:pass@localhost/db"), DbKind::Postgres);
        assert_eq!(detect_db_kind("postgresql://user:pass@localhost/db"), DbKind::Postgres);
        assert_eq!(detect_db_kind("mysql://user:pass@localhost:3306/db"), DbKind::MySql);
        assert_eq!(detect_db_kind("mariadb://user:pass@localhost:3306/db"), DbKind::MySql);
        assert_eq!(detect_db_kind("data/filecodebox.db"), DbKind::Sqlite);
        assert_eq!(detect_db_kind("sqlite:data/filecodebox.db"), DbKind::Sqlite);
        assert_eq!(detect_db_kind(":memory:"), DbKind::Sqlite);
    }
}
