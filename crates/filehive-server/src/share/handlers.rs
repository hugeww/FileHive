use std::collections::HashMap;
use axum::{
    body::Body,
    extract::{FromRequest, Multipart, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Json, Response},
};
use bytes::Bytes;
use chrono::{Duration, Utc};
use filehive_core::{generate_code, sanitize_filename, APIResponse, AppError};
use filehive_db::crud::{consume_file_code, get_file_code_by_code, insert_file_code, NewFileCode};
use filehive_security::{get_select_token, verify_select_token};
use mime_guess::from_path;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::json;
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct ShareTextForm {
    pub text: String,
    #[serde(default = "default_expire_value")]
    pub expire_value: i32,
    #[serde(default = "default_expire_style")]
    pub expire_style: String,
}

#[allow(dead_code)]
fn default_expire_value() -> i32 {
    1
}
#[allow(dead_code)]
fn default_expire_style() -> String {
    "day".to_string()
}

fn calculate_expiration(style: &str, value: i32) -> (Option<chrono::DateTime<Utc>>, i32) {
    let now = Utc::now();
    match style {
        "minute" => (Some(now + Duration::minutes(value as i64)), -1),
        "hour" => (Some(now + Duration::hours(value as i64)), -1),
        "day" => (Some(now + Duration::days(value as i64)), -1),
        "count" => (None, value),
        "forever" => (None, -1),
        _ => (Some(now + Duration::days(1)), -1),
    }
}

pub async fn share_text_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: axum::extract::Request,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let settings = state.config.get();

    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let (text, expire_value, expire_style) = if content_type.starts_with("multipart/form-data") {
        let mut multipart = Multipart::from_request(req, &state)
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let mut text = String::new();
        let mut expire_value = 1;
        let mut expire_style = "day".to_string();

        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?
        {
            let name = field.name().unwrap_or("").to_string();
            if name == "text" {
                text = field.text().await.unwrap_or_default();
            } else if name == "expire_value" {
                let v = field.text().await.unwrap_or_default();
                expire_value = v.parse().unwrap_or(1);
            } else if name == "expire_style" {
                expire_style = field.text().await.unwrap_or_else(|_| "day".to_string());
            }
        }
        (text, expire_value, expire_style)
    } else {
        let bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?;

        if content_type.contains("application/json") {
            let val: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|e| AppError::BadRequest(format!("JSON解析错误: {}", e)))?;
            let text = val.get("text").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let expire_value = val.get("expire_value").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
            let expire_style = val.get("expire_style").and_then(|v| v.as_str()).unwrap_or("day").to_string();
            (text, expire_value, expire_style)
        } else {
            let mut map: HashMap<String, String> = HashMap::new();
            for (k, v) in url::form_urlencoded::parse(&bytes) {
                map.insert(k.to_string(), v.to_string());
            }
            let text = map.remove("text").unwrap_or_default();
            let expire_value = map.remove("expire_value").and_then(|v| v.parse().ok()).unwrap_or(1);
            let expire_style = map.remove("expire_style").unwrap_or_else(|| "day".to_string());
            (text, expire_value, expire_style)
        }
    };

    // 校验文本大小 (<= 222KB)
    let bytes_len = text.as_bytes().len();
    if bytes_len > 222 * 1024 {
        return Err(AppError::Forbidden("内容过多,建议采用文件形式".to_string()));
    }

    let code = generate_code(&settings.code_generate_type);
    let (expired_at, expired_count) = calculate_expiration(&expire_style, expire_value);

    let new_file = NewFileCode {
        code: code.clone(),
        prefix: String::new(),
        suffix: String::new(),
        uuid_file_name: None,
        file_path: None,
        size: bytes_len as i64,
        text: Some(text),
        expired_at,
        expired_count,
        used_count: 0,
        file_hash: None,
        is_chunked: false,
        upload_id: None,
    };

    insert_file_code(&state.pool, &new_file).await?;

    Ok(Json(APIResponse::success(json!({ "code": code }))))
}

pub async fn share_file_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let settings = state.config.get();

    let mut expire_value = 1;
    let mut expire_style = "day".to_string();
    let mut file_bytes: Option<Bytes> = None;
    let mut original_name = "file.bin".to_string();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "expire_value" {
            let val_str = field.text().await.unwrap_or_default();
            expire_value = val_str.parse().unwrap_or(1);
        } else if name == "expire_style" {
            expire_style = field.text().await.unwrap_or_else(|_| "day".to_string());
        } else if name == "file" {
            if let Some(filename) = field.file_name() {
                original_name = filename.to_string();
            }
            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(e.to_string()))?;
            file_bytes = Some(data);
        }
    }

    let data = file_bytes.ok_or_else(|| AppError::BadRequest("未找到上传文件".to_string()))?;
    let size = data.len() as i64;

    if size > settings.upload_size {
        return Err(AppError::PayloadTooLarge(format!(
            "文件过大，超出限制：{}字节",
            settings.upload_size
        )));
    }

    let clean_filename = sanitize_filename(&original_name);
    let uuid_name = format!("{}_{}", uuid::Uuid::new_v4(), clean_filename);
    let date_dir = Utc::now().format("%Y/%m/%d").to_string();
    let save_rel_path = format!("{}/{}", date_dir, uuid_name);

    // 保存至存储引擎
    state.storage.save_file(&save_rel_path, data, None).await?;

    let code = generate_code(&settings.code_generate_type);
    let (expired_at, expired_count) = calculate_expiration(&expire_style, expire_value);

    let new_file = NewFileCode {
        code: code.clone(),
        prefix: date_dir.clone(),
        suffix: clean_filename.clone(),
        uuid_file_name: Some(uuid_name),
        file_path: Some(date_dir),
        size,
        text: None,
        expired_at,
        expired_count,
        used_count: 0,
        file_hash: None,
        is_chunked: false,
        upload_id: None,
    };

    insert_file_code(&state.pool, &new_file).await?;

    Ok(Json(APIResponse::success(json!({
        "code": code,
        "name": clean_filename,
        "size": size
    }))))
}

#[derive(Debug, Deserialize)]
pub struct CodeQuery {
    pub code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CodeBody {
    pub code: Option<String>,
}

pub async fn select_get_handler(
    State(state): State<AppState>,
    Query(query): Query<CodeQuery>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let code = query.code.unwrap_or_default().trim().to_string();
    process_select(state, code).await
}

pub async fn select_post_handler(
    State(state): State<AppState>,
    Json(body): Json<CodeBody>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let code = body.code.unwrap_or_default().trim().to_string();
    process_select(state, code).await
}

async fn process_select(
    state: AppState,
    code: String,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    if code.is_empty() {
        return Err(AppError::BadRequest("请输入取件码".to_string()));
    }

    let item = get_file_code_by_code(&state.pool, &code)
        .await?
        .ok_or_else(|| AppError::NotFound("取件码不存在或已过期".to_string()))?;

    if item.is_expired(Utc::now()) {
        return Err(AppError::NotFound("取件码已过期".to_string()));
    }

    // 原子消费
    consume_file_code(&state.pool, item.id, Utc::now()).await?;

    let settings = state.config.get();

    if let Some(txt) = item.text {
        Ok(Json(APIResponse::success(json!({
            "code": code,
            "type": "text",
            "text": txt
        }))))
    } else {
        let token = get_select_token(&code, &settings.jwt_secret, 0);
        let download_url = format!("/share/download?key={}&code={}", token, code);
        Ok(Json(APIResponse::success(json!({
            "code": code,
            "type": "file",
            "name": item.suffix,
            "size": item.size,
            "download_url": download_url
        }))))
    }
}

#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    pub code: Option<String>,
    pub key: Option<String>,
}

pub async fn download_handler(
    State(state): State<AppState>,
    Query(query): Query<DownloadQuery>,
) -> Result<Response, AppError> {
    let code = query.code.unwrap_or_default().trim().to_string();
    let key = query.key.unwrap_or_default().trim().to_string();

    let settings = state.config.get();
    if !verify_select_token(&code, &key, &settings.jwt_secret) {
        return Err(AppError::Forbidden("下载凭据无效或已过期".to_string()));
    }

    let item = get_file_code_by_code(&state.pool, &code)
        .await?
        .ok_or_else(|| AppError::NotFound("文件不存在".to_string()))?;

    let full_path = item.get_file_path();
    let data = state.storage.read_file(&full_path).await?;

    let mime_type = from_path(&item.suffix)
        .first_or_octet_stream()
        .to_string();

    let encoded_filename = utf8_percent_encode(&item.suffix, NON_ALPHANUMERIC).to_string();
    let disposition = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        item.suffix, encoded_filename
    );

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CONTENT_DISPOSITION, disposition)
        .header(header::CONTENT_LENGTH, data.len().to_string())
        .body(Body::from(data))
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(response)
}

pub async fn metadata_get_handler(
    State(state): State<AppState>,
    Query(query): Query<CodeQuery>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let code = query.code.unwrap_or_default().trim().to_string();
    process_metadata(state, code).await
}

pub async fn metadata_post_handler(
    State(state): State<AppState>,
    Json(body): Json<CodeBody>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let code = body.code.unwrap_or_default().trim().to_string();
    process_metadata(state, code).await
}

async fn process_metadata(
    state: AppState,
    code: String,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    if code.is_empty() {
        return Err(AppError::BadRequest("请输入取件码".to_string()));
    }

    let item = get_file_code_by_code(&state.pool, &code)
        .await?
        .ok_or_else(|| AppError::NotFound("取件码不存在".to_string()))?;

    let is_text = item.text.is_some();
    Ok(Json(APIResponse::success(json!({
        "code": code,
        "type": if is_text { "text" } else { "file" },
        "name": item.suffix,
        "size": item.size,
        "expired_at": item.expired_at,
        "used_count": item.used_count,
        "expired_count": item.expired_count
    }))))
}
