//! # fcb-core
//!
//! Core types, models, errors, response envelopes, and utility functions
//! for the FileHive Rust rewrite.

pub mod errors;
pub mod models;
pub mod response;
pub mod utils;
pub mod version;

// Re-exports
pub use errors::AppError;
pub use models::StoredFile;
pub use response::{APIResponse, ErrorResponse};
pub use utils::{generate_code, sanitize_filename};
pub use version::APP_VERSION;
