//! Application errors for FileHive.

use thiserror::Error;

use crate::response::ErrorResponse;

/// Main error type representing application-level errors.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    /// Storage backend error with an HTTP-compatible status code and detail message.
    #[error("StorageError ({status_code}): {detail}")]
    StorageError { status_code: u16, detail: String },

    /// Resource not found (HTTP 404).
    #[error("Not Found: {0}")]
    NotFound(String),

    /// Invalid client request (HTTP 400).
    #[error("Bad Request: {0}")]
    BadRequest(String),

    /// Authentication failed or missing (HTTP 401).
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Operation forbidden by policy or permissions (HTTP 403).
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// Upload payload exceeded size limit (HTTP 413).
    #[error("Payload Too Large: {0}")]
    PayloadTooLarge(String),

    /// Precondition required before proceeding (HTTP 428).
    #[error("Precondition Required: {0}")]
    PreconditionRequired(String),

    /// Too many requests (HTTP 429).
    #[error("Too Many Requests: {0}")]
    TooManyRequests(String),

    /// Internal unexpected server error (HTTP 500).
    #[error("Internal Error: {0}")]
    Internal(String),

    /// Database query or connection error (HTTP 500).
    #[error("Database Error: {0}")]
    Database(String),
}

impl AppError {
    /// Returns the numeric HTTP status code associated with this error.
    pub fn status_code(&self) -> u16 {
        match self {
            AppError::StorageError { status_code, .. } => *status_code,
            AppError::NotFound(_) => 404,
            AppError::BadRequest(_) => 400,
            AppError::Unauthorized(_) => 401,
            AppError::Forbidden(_) => 403,
            AppError::PayloadTooLarge(_) => 413,
            AppError::TooManyRequests(_) => 429,
            AppError::PreconditionRequired(_) => 428,
            AppError::Internal(_) => 500,
            AppError::Database(_) => 500,
        }
    }

    /// Returns the `http::StatusCode` representation of this error.
    pub fn http_status(&self) -> http::StatusCode {
        http::StatusCode::from_u16(self.status_code())
            .unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR)
    }

    /// Returns the primary error message string.
    pub fn message(&self) -> String {
        self.detail().to_string()
    }

    /// Returns the detail string slice for this error.
    pub fn detail(&self) -> &str {
        match self {
            AppError::StorageError { detail, .. } => detail.as_str(),
            AppError::NotFound(msg)
            | AppError::BadRequest(msg)
            | AppError::Unauthorized(msg)
            | AppError::Forbidden(msg)
            | AppError::PayloadTooLarge(msg)
            | AppError::TooManyRequests(msg)
            | AppError::PreconditionRequired(msg)
            | AppError::Internal(msg)
            | AppError::Database(msg) => msg.as_str(),
        }
    }

    /// Helper constructor for `NotFound`.
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Helper constructor for `BadRequest`.
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    /// Helper constructor for `Unauthorized`.
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Unauthorized(msg.into())
    }

    /// Helper constructor for `Forbidden`.
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }

    /// Helper constructor for `PayloadTooLarge`.
    pub fn payload_too_large(msg: impl Into<String>) -> Self {
        Self::PayloadTooLarge(msg.into())
    }

    /// Helper constructor for `PreconditionRequired`.
    pub fn precondition_required(msg: impl Into<String>) -> Self {
        Self::PreconditionRequired(msg.into())
    }

    pub fn too_many_requests(msg: impl Into<String>) -> Self {
        Self::TooManyRequests(msg.into())
    }

    /// Helper constructor for `Internal`.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Helper constructor for `Database`.
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }

    /// Helper constructor for `StorageError`.
    pub fn storage(status_code: u16, detail: impl Into<String>) -> Self {
        Self::StorageError {
            status_code,
            detail: detail.into(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

impl From<&AppError> for ErrorResponse {
    fn from(err: &AppError) -> Self {
        Self {
            code: err.status_code() as i32,
            message: err.to_string(),
            detail: serde_json::Value::String(err.detail().to_string()),
        }
    }
}

impl From<AppError> for ErrorResponse {
    fn from(err: AppError) -> Self {
        ErrorResponse::from(&err)
    }
}

#[cfg(feature = "axum")]
impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status = self.http_status();
        let body = serde_json::json!({
            "detail": self.detail()
        });
        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_codes() {
        assert_eq!(AppError::NotFound("x".into()).status_code(), 404);
        assert_eq!(AppError::BadRequest("x".into()).status_code(), 400);
        assert_eq!(AppError::Unauthorized("x".into()).status_code(), 401);
        assert_eq!(AppError::Forbidden("x".into()).status_code(), 403);
        assert_eq!(AppError::PayloadTooLarge("x".into()).status_code(), 413);
        assert_eq!(AppError::PreconditionRequired("x".into()).status_code(), 428);
        assert_eq!(AppError::Internal("x".into()).status_code(), 500);
        assert_eq!(AppError::Database("x".into()).status_code(), 500);
        assert_eq!(
            AppError::StorageError {
                status_code: 507,
                detail: "quota full".into(),
            }
            .status_code(),
            507
        );
    }

    #[test]
    fn test_error_display() {
        let err = AppError::NotFound("File not found".into());
        assert_eq!(err.to_string(), "Not Found: File not found");
        assert_eq!(err.detail(), "File not found");
        assert_eq!(err.message(), "File not found");
    }

    #[test]
    fn test_into_error_response() {
        let err = AppError::NotFound("File not found".into());
        let resp: ErrorResponse = err.into();
        assert_eq!(resp.code, 404);
        assert_eq!(resp.message, "Not Found: File not found");
        assert_eq!(resp.detail, serde_json::json!("File not found"));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "disk read failure");
        let app_err = AppError::from(io_err);
        assert_eq!(app_err.status_code(), 500);
        assert!(app_err.detail().contains("disk read failure"));
    }
}
