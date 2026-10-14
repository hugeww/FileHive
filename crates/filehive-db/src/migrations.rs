use filehive_core::AppError;
use sqlx::{MySqlPool, PgPool, SqlitePool};
use crate::pool::DbPool;

/// 运行数据库迁移，根据连接池底层类型分发
pub async fn run_migrations(pool: &DbPool) -> Result<(), AppError> {
    match pool {
        DbPool::Sqlite(p) => run_sqlite_migrations(p).await,
        DbPool::Postgres(p) => run_postgres_migrations(p).await,
        DbPool::MySql(p) => run_mysql_migrations(p).await,
    }
}

pub async fn run_sqlite_migrations(pool: &SqlitePool) -> Result<(), AppError> {
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
    .map_err(|e| AppError::Database(format!("Failed to run SQLite migrations: {}", e)))?;

    Ok(())
}

pub async fn run_postgres_migrations(pool: &PgPool) -> Result<(), AppError> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS filecodes (
            id BIGSERIAL PRIMARY KEY,
            code VARCHAR(255) NOT NULL UNIQUE,
            prefix VARCHAR(255) DEFAULT '',
            suffix VARCHAR(255) DEFAULT '',
            uuid_file_name VARCHAR(255),
            file_path VARCHAR(255),
            size BIGINT DEFAULT 0,
            text TEXT,
            expired_at TIMESTAMPTZ,
            expired_count INT DEFAULT -1,
            used_count INT DEFAULT 0,
            created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
            file_hash VARCHAR(64),
            is_chunked BOOLEAN DEFAULT FALSE,
            upload_id VARCHAR(36)
        );
        CREATE INDEX IF NOT EXISTS idx_filecodes_code ON filecodes(code);

        CREATE TABLE IF NOT EXISTS uploadchunk (
            id BIGSERIAL PRIMARY KEY,
            upload_id VARCHAR(36) NOT NULL,
            chunk_index INT NOT NULL,
            chunk_hash VARCHAR(64) NOT NULL,
            total_chunks INT NOT NULL,
            file_size BIGINT NOT NULL,
            chunk_size INT NOT NULL,
            file_name VARCHAR(255) NOT NULL,
            save_path VARCHAR(512),
            created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
            completed BOOLEAN DEFAULT FALSE
        );
        CREATE INDEX IF NOT EXISTS idx_uploadchunk_upload_id ON uploadchunk(upload_id);

        CREATE TABLE IF NOT EXISTS keyvalue (
            id BIGSERIAL PRIMARY KEY,
            key VARCHAR(255) NOT NULL UNIQUE,
            value TEXT,
            created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_keyvalue_key ON keyvalue(key);

        CREATE TABLE IF NOT EXISTS presignuploadsession (
            id BIGSERIAL PRIMARY KEY,
            upload_id VARCHAR(36) NOT NULL UNIQUE,
            file_name VARCHAR(255) NOT NULL,
            file_size BIGINT NOT NULL,
            save_path VARCHAR(512) NOT NULL,
            mode VARCHAR(10) NOT NULL,
            expire_value INT DEFAULT 1,
            expire_style VARCHAR(20) DEFAULT 'day',
            created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
            expires_at TIMESTAMPTZ NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_presignuploadsession_upload_id ON presignuploadsession(upload_id);

        CREATE TABLE IF NOT EXISTS storagereservation (
            id BIGSERIAL PRIMARY KEY,
            token VARCHAR(64) NOT NULL UNIQUE,
            size BIGINT NOT NULL,
            expires_at TIMESTAMPTZ NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_storagereservation_token ON storagereservation(token);
        CREATE INDEX IF NOT EXISTS idx_storagereservation_expires_at ON storagereservation(expires_at);

        CREATE TABLE IF NOT EXISTS migrates (
            id BIGSERIAL PRIMARY KEY,
            migration_file VARCHAR(255) NOT NULL UNIQUE,
            executed_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to run PostgreSQL migrations: {}", e)))?;

    Ok(())
}

pub async fn run_mysql_migrations(pool: &MySqlPool) -> Result<(), AppError> {
    let queries = [
        r#"
        CREATE TABLE IF NOT EXISTS filecodes (
            id BIGINT PRIMARY KEY AUTO_INCREMENT,
            code VARCHAR(255) NOT NULL UNIQUE,
            prefix VARCHAR(255) DEFAULT '',
            suffix VARCHAR(255) DEFAULT '',
            uuid_file_name VARCHAR(255),
            file_path VARCHAR(255),
            size BIGINT DEFAULT 0,
            text LONGTEXT,
            expired_at DATETIME(6),
            expired_count INT DEFAULT -1,
            used_count INT DEFAULT 0,
            created_at DATETIME(6) DEFAULT CURRENT_TIMESTAMP(6),
            file_hash VARCHAR(64),
            is_chunked BOOLEAN DEFAULT 0,
            upload_id VARCHAR(36),
            INDEX idx_filecodes_code (code)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS uploadchunk (
            id BIGINT PRIMARY KEY AUTO_INCREMENT,
            upload_id VARCHAR(36) NOT NULL,
            chunk_index INT NOT NULL,
            chunk_hash VARCHAR(64) NOT NULL,
            total_chunks INT NOT NULL,
            file_size BIGINT NOT NULL,
            chunk_size INT NOT NULL,
            file_name VARCHAR(255) NOT NULL,
            save_path VARCHAR(512),
            created_at DATETIME(6) DEFAULT CURRENT_TIMESTAMP(6),
            completed BOOLEAN DEFAULT 0,
            INDEX idx_uploadchunk_upload_id (upload_id)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS keyvalue (
            id BIGINT PRIMARY KEY AUTO_INCREMENT,
            `key` VARCHAR(255) NOT NULL UNIQUE,
            `value` LONGTEXT,
            created_at DATETIME(6) DEFAULT CURRENT_TIMESTAMP(6),
            INDEX idx_keyvalue_key (`key`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS presignuploadsession (
            id BIGINT PRIMARY KEY AUTO_INCREMENT,
            upload_id VARCHAR(36) NOT NULL UNIQUE,
            file_name VARCHAR(255) NOT NULL,
            file_size BIGINT NOT NULL,
            save_path VARCHAR(512) NOT NULL,
            mode VARCHAR(10) NOT NULL,
            expire_value INT DEFAULT 1,
            expire_style VARCHAR(20) DEFAULT 'day',
            created_at DATETIME(6) DEFAULT CURRENT_TIMESTAMP(6),
            expires_at DATETIME(6) NOT NULL,
            INDEX idx_presignuploadsession_upload_id (upload_id)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS storagereservation (
            id BIGINT PRIMARY KEY AUTO_INCREMENT,
            token VARCHAR(64) NOT NULL UNIQUE,
            size BIGINT NOT NULL,
            expires_at DATETIME(6) NOT NULL,
            INDEX idx_storagereservation_token (token),
            INDEX idx_storagereservation_expires_at (expires_at)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS migrates (
            id BIGINT PRIMARY KEY AUTO_INCREMENT,
            migration_file VARCHAR(255) NOT NULL UNIQUE,
            executed_at DATETIME(6) DEFAULT CURRENT_TIMESTAMP(6)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
        "#,
    ];

    for q in queries {
        sqlx::query(q)
            .execute(pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to run MySQL migration query: {}", e)))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migrations_create_tables() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let db_pool = DbPool::Sqlite(pool);
        run_migrations(&db_pool).await.unwrap();

        let p = db_pool.as_sqlite().unwrap();
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
        )
        .fetch_all(p)
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
