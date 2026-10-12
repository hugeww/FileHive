use fcb_core::AppError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

/// Initialize the SQLite database connection pool and run migrations.
///
/// Ensures:
/// - WAL journal mode (`PRAGMA journal_mode = WAL`)
/// - Busy timeout set to 10 seconds (`PRAGMA busy_timeout = 10000`)
/// - Foreign keys enabled (`PRAGMA foreign_keys = ON`)
/// - Database file creation if missing
/// - Migrations are run upon initialization
pub async fn init_pool(db_path: &str) -> Result<SqlitePool, AppError> {
    // If the path doesn't specify a sqlite scheme or :memory:, prepend sqlite:
    // SqliteConnectOptions::from_str handles paths, but if db_path is e.g. "data/filecodebox.db",
    // from_str parses it smoothly.
    let connect_opts = SqliteConnectOptions::from_str(db_path)
        .map_err(|e| AppError::Database(format!("Invalid SQLite connection path '{}': {}", db_path, e)))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_millis(10000))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(20)
        .connect_with(connect_opts)
        .await
        .map_err(|e| AppError::Database(format!("Failed to connect to SQLite: {}", e)))?;

    // Run table initialization and migrations
    crate::migrations::run_migrations(&pool).await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_pool_in_memory() {
        let pool = init_pool(":memory:").await.expect("Failed to init in-memory pool");

        // Verify WAL or in-memory journal mode
        let journal_mode: (String,) = sqlx::query_as("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .unwrap();
        // In-memory sqlite often reports "memory" for journal_mode even if WAL requested
        assert!(journal_mode.0.eq_ignore_ascii_case("memory") || journal_mode.0.eq_ignore_ascii_case("wal"));

        // Verify busy timeout
        let busy_timeout: (i64,) = sqlx::query_as("PRAGMA busy_timeout")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(busy_timeout.0, 10000);

        // Verify foreign keys
        let foreign_keys: (i64,) = sqlx::query_as("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(foreign_keys.0, 1);
    }
}
