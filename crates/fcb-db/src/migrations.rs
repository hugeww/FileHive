use fcb_core::AppError;
use sqlx::SqlitePool;

/// Initialize and execute schema migrations.
/// Creates all required tables and indexes if they do not exist:
/// - `filecodes`
/// - `uploadchunk`
/// - `keyvalue`
/// - `presignuploadsession`
/// - `storagereservation`
/// - `migrates`
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), AppError> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS filecodes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            code TEXT UNIQUE,
            prefix TEXT DEFAULT '',
            suffix TEXT DEFAULT '',
            uuid_file_name TEXT,
            file_path TEXT,
            size INTEGER DEFAULT 0,
            text TEXT,
            expired_at TIMESTAMP,
            expired_count INTEGER DEFAULT -1,
            used_count INTEGER DEFAULT 0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            file_hash TEXT,
            is_chunked BOOLEAN DEFAULT 0,
            upload_id TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_filecodes_code ON filecodes(code);

        CREATE TABLE IF NOT EXISTS uploadchunk (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            upload_id TEXT,
            chunk_index INTEGER,
            chunk_hash TEXT,
            total_chunks INTEGER,
            file_size INTEGER,
            chunk_size INTEGER,
            file_name TEXT,
            save_path TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            completed BOOLEAN DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_uploadchunk_upload_id ON uploadchunk(upload_id);

        CREATE TABLE IF NOT EXISTS keyvalue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            key TEXT UNIQUE,
            value TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_keyvalue_key ON keyvalue(key);

        CREATE TABLE IF NOT EXISTS presignuploadsession (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            upload_id TEXT UNIQUE,
            file_name TEXT,
            file_size INTEGER,
            save_path TEXT,
            mode TEXT,
            expire_value INTEGER DEFAULT 1,
            expire_style TEXT DEFAULT 'day',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            expires_at TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_presignuploadsession_upload_id ON presignuploadsession(upload_id);

        CREATE TABLE IF NOT EXISTS storagereservation (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            token TEXT UNIQUE,
            size INTEGER,
            expires_at TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_storagereservation_token ON storagereservation(token);
        CREATE INDEX IF NOT EXISTS idx_storagereservation_expires_at ON storagereservation(expires_at);

        CREATE TABLE IF NOT EXISTS migrates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            migration_file TEXT UNIQUE,
            executed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to run migrations: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migrations_create_tables() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();

        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let table_names: Vec<String> = tables.into_iter().map(|t| t.0).collect();
        assert!(table_names.contains(&"filecodes".to_string()));
        assert!(table_names.contains(&"uploadchunk".to_string()));
        assert!(table_names.contains(&"keyvalue".to_string()));
        assert!(table_names.contains(&"presignuploadsession".to_string()));
        assert!(table_names.contains(&"storagereservation".to_string()));
        assert!(table_names.contains(&"migrates".to_string()));
    }
}
