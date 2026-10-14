//! API response models for FileHive.

use serde::{Deserialize, Serialize};

fn default_code() -> i32 {
    200
}

fn default_success_msg() -> String {
    "success".to_string()
}

/// Unified API response envelope for FileHive endpoints.
///
/// Contains both `message` and `msg` fields initialized to "success" by default,
/// ensuring compatibility with various frontend clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct APIResponse<T> {
    /// Status code (default 200).
    #[serde(default = "default_code")]
    pub code: i32,

    /// Response message (default "success").
    #[serde(default = "default_success_msg")]
    pub message: String,

    /// Alias for message (default "success").
    #[serde(default = "default_success_msg")]
    pub msg: String,

    /// Detailed response payload.
    pub detail: T,
}

impl<T> APIResponse<T> {
    /// Create a successful response (code 200, message "success") with the provided detail.
    pub fn success(detail: T) -> Self {
        Self {
            code: 200,
            message: "success".to_string(),
            msg: "success".to_string(),
            detail,
        }
    }

    /// Create a response with a custom status code, message, and detail payload.
    ///
    /// Both `message` and `msg` are set to `msg`.
    pub fn new(code: i32, msg: &str, detail: T) -> Self {
        Self {
            code,
            message: msg.to_string(),
            msg: msg.to_string(),
            detail,
        }
    }
}

impl<T: Default> Default for APIResponse<T> {
    fn default() -> Self {
        Self::success(T::default())
    }
}

/// Standard error response envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error status code.
    pub code: i32,

    /// Error message.
    pub message: String,

    /// Detailed error payload or message.
    pub detail: serde_json::Value,
}

impl ErrorResponse {
    /// Create a new error response with status code, message, and detail value.
    pub fn new(code: i32, message: impl Into<String>, detail: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            detail,
        }
    }

    /// Create an error response from an error message, setting detail to the string as well.
    pub fn from_message(code: i32, message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            code,
            message: msg.clone(),
            detail: serde_json::Value::String(msg),
        }
    }
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for ErrorResponse {}

#[cfg(feature = "axum")]
impl<T: Serialize> axum::response::IntoResponse for APIResponse<T> {
    fn into_response(self) -> axum::response::Response {
        axum::Json(self).into_response()
    }
}

#[cfg(feature = "axum")]
impl axum::response::IntoResponse for ErrorResponse {
    fn into_response(self) -> axum::response::Response {
        let status = http::StatusCode::from_u16(self.code as u16)
            .unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR);
        (status, axum::Json(self)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_api_response_success() {
        let res = APIResponse::success("payload");
        assert_eq!(res.code, 200);
        assert_eq!(res.message, "success");
        assert_eq!(res.msg, "success");
        assert_eq!(res.detail, "payload");

        let serialized = serde_json::to_string(&res).unwrap();
        assert!(serialized.contains("\"code\":200"));
        assert!(serialized.contains("\"message\":\"success\""));
        assert!(serialized.contains("\"msg\":\"success\""));
        assert!(serialized.contains("\"detail\":\"payload\""));
    }

    #[test]
    fn test_api_response_new() {
        let res = APIResponse::new(201, "created", 123);
        assert_eq!(res.code, 201);
        assert_eq!(res.message, "created");
        assert_eq!(res.msg, "created");
        assert_eq!(res.detail, 123);
    }

    #[test]
    fn test_api_response_deserialize_defaults() {
        let json_str = r#"{"detail": "only detail"}"#;
        let res: APIResponse<String> = serde_json::from_str(json_str).unwrap();
        assert_eq!(res.code, 200);
        assert_eq!(res.message, "success");
        assert_eq!(res.msg, "success");
        assert_eq!(res.detail, "only detail");
    }

    #[test]
    fn test_error_response_new() {
        let err = ErrorResponse::new(400, "bad request", json!({"reason": "invalid_code"}));
        assert_eq!(err.code, 400);
        assert_eq!(err.message, "bad request");
        assert_eq!(err.detail["reason"], "invalid_code");
    }

    #[test]
    fn test_error_response_from_message() {
        let err = ErrorResponse::from_message(404, "file not found");
        assert_eq!(err.code, 404);
        assert_eq!(err.message, "file not found");
        assert_eq!(err.detail, json!("file not found"));
        assert_eq!(err.to_string(), "Error 404: file not found");
    }
}
