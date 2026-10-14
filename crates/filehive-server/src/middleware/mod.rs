use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Redirect, Response},
};
use filehive_core::ErrorResponse;
use serde_json::json;
use crate::state::AppState;

pub async fn setup_guard_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path();
    let settings = state.config.get();

    let is_initialized = !settings.admin_token.is_empty();

    if !is_initialized && !path.starts_with("/setup") && !path.starts_with("/assets") && path != "/health" {
        let accept = req
            .headers()
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if accept.contains("text/html") {
            return Redirect::to("/setup").into_response();
        } else {
            let err = ErrorResponse::new(
                428,
                "系统未初始化，请先完成初始化",
                json!({ "setup": "/setup" }),
            );
            return (StatusCode::PRECONDITION_REQUIRED, Json(err)).into_response();
        }
    }

    next.run(req).await
}
