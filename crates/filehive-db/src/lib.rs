//! `fcb-db` crate: Database models, pool management, migrations, and CRUD operations for FileHive.

pub mod crud;
pub mod migrations;
pub mod models;
pub mod pool;

pub use crud::*;
pub use migrations::run_migrations;
pub use models::*;
pub use pool::{detect_db_kind, init_pool, DbKind, DbPool};
