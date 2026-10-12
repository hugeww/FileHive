use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
};
use fcb_core::{version::APP_VERSION, APIResponse, AppError};
use mime_guess::from_path;
use serde_json::json;
use std::path::PathBuf;

use crate::state::AppState;

pub fn build_public_config(state: &AppState) -> serde_json::Value {
    let s = state.config.get();
    json!({
        "name": s.name,
        "description": s.description,
        "explain": s.page_explain,
        "upload_size": s.upload_size,
        "allowed_file_types": s.allowed_file_types,
        "expire_style": s.expire_style,
        "enable_chunk": s.enable_chunk,
        "open_upload": s.open_upload,
        "notify_title": s.notify_title,
        "notify_content": s.notify_content,
        "show_admin_address": s.show_admin_addr,
        "max_save_seconds": 0
    })
}

pub fn build_public_meta(state: &AppState) -> serde_json::Value {
    let s = state.config.get();
    json!({
        "version": APP_VERSION,
        "api": {
            "legacy_config": "/",
            "public_config": "/api/v1/config",
            "health": "/health"
        },
        "features": {
            "chunk_upload": s.enable_chunk > 0,
            "guest_upload": s.open_upload > 0,
            "admin_address_visible": s.show_admin_addr > 0,
            "expiration_modes": s.expire_style
        },
        "limits": {
            "upload_size": s.upload_size,
            "allowed_file_types": s.allowed_file_types,
            "max_save_seconds": 0,
            "upload_window_minutes": s.upload_minute,
            "upload_window_count": s.upload_count
        }
    })
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

pub async fn index_handler(State(state): State<AppState>) -> Response {
    let settings = state.config.get();
    let theme_dir = PathBuf::from(&settings.themes_select);
    let index_file = theme_dir.join("index.html");

    let raw_html = if index_file.exists() {
        std::fs::read_to_string(&index_file).unwrap_or_else(|_| default_html())
    } else {
        default_html()
    };

    let rendered = raw_html
        .replace("{{title}}", &escape_html(&settings.name))
        .replace("{{description}}", &escape_html(&settings.description))
        .replace("{{keywords}}", &escape_html(&settings.keywords))
        .replace("{{opacity}}", &settings.opacity.to_string())
        .replace("{{background}}", &settings.background);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(rendered))
        .unwrap()
}

pub async fn post_config_handler(
    State(state): State<AppState>,
) -> Json<APIResponse<serde_json::Value>> {
    Json(APIResponse::success(build_public_config(&state)))
}

pub async fn get_public_config_handler(
    State(state): State<AppState>,
) -> Json<APIResponse<serde_json::Value>> {
    Json(APIResponse::success(json!({
        "config": build_public_config(&state),
        "meta": build_public_meta(&state)
    })))
}

pub async fn health_handler(
    State(state): State<AppState>,
) -> Json<APIResponse<serde_json::Value>> {
    let s = state.config.get();
    Json(APIResponse::success(json!({
        "status": "ok",
        "version": APP_VERSION,
        "storage": s.file_storage,
        "theme": s.themes_select
    })))
}

pub async fn robots_handler(State(state): State<AppState>) -> Response {
    let s = state.config.get();
    Response::builder()
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from(s.robots_text.clone()))
        .unwrap()
}

pub async fn theme_asset_handler(
    State(state): State<AppState>,
    Path(asset_path): Path<String>,
) -> Result<Response, AppError> {
    let s = state.config.get();
    let theme_dir = PathBuf::from(&s.themes_select);
    let full_path = theme_dir.join("assets").join(&asset_path);

    if !full_path.exists() || !full_path.is_file() {
        return Err(AppError::NotFound("资源不存在".to_string()));
    }

    let bytes = std::fs::read(&full_path).map_err(|e| AppError::Internal(e.to_string()))?;
    let mime = from_path(&full_path).first_or_octet_stream().to_string();

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .body(Body::from(bytes))
        .unwrap())
}

pub async fn spa_fallback_handler(
    State(state): State<AppState>,
    req: Request,
) -> Response {
    let accept = req
        .headers()
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if (req.method() == axum::http::Method::GET || req.method() == axum::http::Method::HEAD)
        && accept.contains("text/html")
    {
        index_handler(State(state)).await
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({
                "code": 404,
                "message": "Not Found",
                "detail": "资源不存在"
            })),
        )
            .into_response()
    }
}

fn default_html() -> String {
    r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <title>{{title}}</title>
    <meta name="description" content="{{description}}">
    <meta name="keywords" content="{{keywords}}">
    <style>
        body { font-family: -apple-system, sans-serif; display: flex; justify-content: center; align-items: center; min-height: 100vh; margin: 0; background: #fafafa; }
        .box { text-align: center; padding: 40px; background: white; border-radius: 12px; box-shadow: 0 4px 20px rgba(0,0,0,0.06); }
        h1 { font-size: 24px; color: #111; margin-bottom: 12px; }
        p { color: #666; margin-bottom: 24px; }
    </style>
</head>
<body>
    <div class="box">
        <h1>{{title}}</h1>
        <p>{{description}}</p>
        <div>FileCodeBox-rs 已经就绪并正常运行</div>
    </div>
</body>
</html>"#
    .to_string()
}
