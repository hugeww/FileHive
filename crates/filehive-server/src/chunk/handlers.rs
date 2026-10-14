use std::collections::HashMap;
use axum::{
    body::Bytes,
    extract::{Multipart, Path, State},
    http::{header, HeaderMap},
    response::Json,
};
use chrono::Utc;
use filehive_config::quota::{release_storage, reserve_storage};
use filehive_core::{generate_code, sanitize_filename, APIResponse, AppError};
use filehive_db::crud::{
    delete_upload_chunks, get_chunk_session, get_uploaded_chunk_indexes, insert_chunk_session,
    insert_file_code, mark_chunk_completed, NewFileCode,
};
use serde::Deserialize;
use serde_json::json;
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct InitChunkUploadModel {
    pub file_name: String,
    pub file_size: i64,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: i32,
    #[serde(default)]
    pub total_chunks: Option<i32>,
    #[serde(default)]
    pub file_hash: Option<String>,
    #[serde(default = "default_expire_val")]
    pub expire_value: i32,
    #[serde(default = "default_expire_sty")]
    pub expire_style: String,
}

fn default_chunk_size() -> i32 {
    5 * 1024 * 1024
}
fn default_expire_val() -> i32 {
    1
}
fn default_expire_sty() -> String {
    "day".to_string()
}

pub async fn chunk_init_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: axum::extract::Request,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let settings = state.config.get();

    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let payload: InitChunkUploadModel = if content_type.contains("application/json") {
        let bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        serde_json::from_slice(&bytes).map_err(|e| AppError::BadRequest(e.to_string()))?
    } else {
        let bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let mut map: HashMap<String, String> = HashMap::new();
        for (k, v) in url::form_urlencoded::parse(&bytes) {
            map.insert(k.to_string(), v.to_string());
        }
        let file_name = map.remove("file_name").unwrap_or_default();
        let file_size: i64 = map
            .remove("file_size")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let chunk_size: i32 = map
            .remove("chunk_size")
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(default_chunk_size);
        let file_hash = map.remove("file_hash");
        let total_chunks = map.remove("total_chunks").and_then(|v| v.parse().ok());
        let expire_value = map
            .remove("expire_value")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let expire_style = map.remove("expire_style").unwrap_or_else(|| "day".to_string());

        InitChunkUploadModel {
            file_name,
            file_size,
            chunk_size,
            total_chunks,
            file_hash,
            expire_value,
            expire_style,
        }
    };

    if payload.file_size > settings.upload_size {
        return Err(AppError::PayloadTooLarge(format!(
            "文件大小超过限制，最大为 {} MB",
            settings.upload_size / (1024 * 1024)
        )));
    }

    let chunk_size = if payload.chunk_size <= 0 {
        default_chunk_size()
    } else {
        payload.chunk_size
    };
    let total_chunks = payload.total_chunks.unwrap_or_else(|| {
        ((payload.file_size + chunk_size as i64 - 1) / chunk_size as i64).max(1) as i32
    });

    let upload_id = uuid::Uuid::new_v4().simple().to_string();

    // 预留存储容量
    reserve_storage(&state.pool, &upload_id, payload.file_size, 24 * 3600).await?;

    let file_hash = payload.file_hash.unwrap_or_default();

    // 写入 session 记录到 uploadchunk (chunk_index = -1 作为 session 占位)
    insert_chunk_session(
        &state.pool,
        &upload_id,
        &payload.file_name,
        payload.file_size,
        chunk_size,
        total_chunks,
        &file_hash,
    )
    .await?;

    Ok(Json(APIResponse::success(json!({
        "upload_id": upload_id,
        "chunk_size": chunk_size,
        "total_chunks": total_chunks,
        "uploaded_chunks": []
    }))))
}

pub async fn chunk_upload_path_handler(
    State(state): State<AppState>,
    Path((upload_id, chunk_index)): Path<(String, i32)>,
    mut multipart: Multipart,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let mut chunk_bytes: Option<Bytes> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "chunk" || name == "file" {
            chunk_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
            break;
        }
    }

    let data = chunk_bytes.ok_or_else(|| AppError::BadRequest("未接收到分片数据".to_string()))?;
    state
        .storage
        .save_chunk(&upload_id, chunk_index, data, "")
        .await?;

    // 标记分片已完成
    mark_chunk_completed(&state.pool, &upload_id, chunk_index).await?;

    Ok(Json(APIResponse::success(json!({
        "chunk_index": chunk_index,
        "chunk_hash": ""
    }))))
}

pub async fn chunk_upload_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let mut upload_id = String::new();
    let mut chunk_index = 0;
    let mut chunk_bytes: Option<Bytes> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "upload_id" {
            upload_id = field.text().await.unwrap_or_default();
        } else if name == "chunk_index" {
            let idx_str = field.text().await.unwrap_or_default();
            chunk_index = idx_str.parse().unwrap_or(0);
        } else if name == "chunk" || name == "file" {
            chunk_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
    }

    let data = chunk_bytes.ok_or_else(|| AppError::BadRequest("未接收到分片数据".to_string()))?;
    if upload_id.is_empty() {
        return Err(AppError::BadRequest("缺少 upload_id".to_string()));
    }

    state
        .storage
        .save_chunk(&upload_id, chunk_index, data, "")
        .await?;

    mark_chunk_completed(&state.pool, &upload_id, chunk_index).await?;

    Ok(Json(APIResponse::success(json!({ "chunk_index": chunk_index }))))
}

#[derive(Debug, Deserialize, Default)]
pub struct CompleteUploadModel {
    pub upload_id: Option<String>,
    #[serde(default)]
    pub total_chunks: Option<i32>,
    #[serde(default = "default_expire_val")]
    pub expire_value: i32,
    #[serde(default = "default_expire_sty")]
    pub expire_style: String,
}

pub async fn chunk_complete_path_handler(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
    headers: HeaderMap,
    req: axum::extract::Request,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let (expire_value, expire_style) = if content_type.contains("application/json") {
        let bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let m: CompleteUploadModel = serde_json::from_slice(&bytes).unwrap_or_default();
        (m.expire_value, m.expire_style)
    } else {
        let bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let mut map: HashMap<String, String> = HashMap::new();
        for (k, v) in url::form_urlencoded::parse(&bytes) {
            map.insert(k.to_string(), v.to_string());
        }
        let exp_val = map.remove("expire_value").and_then(|v| v.parse().ok()).unwrap_or(1);
        let exp_sty = map.remove("expire_style").unwrap_or_else(|| "day".to_string());
        (exp_val, exp_sty)
    };

    do_complete_chunk(state, upload_id, None, expire_value, expire_style).await
}

pub async fn chunk_complete_handler(
    State(state): State<AppState>,
    Json(payload): Json<CompleteUploadModel>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let upload_id = payload
        .upload_id
        .ok_or_else(|| AppError::BadRequest("缺少 upload_id".to_string()))?;
    do_complete_chunk(
        state,
        upload_id,
        payload.total_chunks,
        payload.expire_value,
        payload.expire_style,
    )
    .await
}

async fn do_complete_chunk(
    state: AppState,
    upload_id: String,
    provided_total_chunks: Option<i32>,
    expire_value: i32,
    expire_style: String,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let session_row = get_chunk_session(&state.pool, &upload_id).await?;

    let (file_name, file_size, _, session_total) =
        session_row.ok_or_else(|| AppError::NotFound("未找到上传任务会话".to_string()))?;

    let total_chunks = provided_total_chunks.unwrap_or(session_total);

    let clean_filename = sanitize_filename(&file_name);
    let uuid_name = format!("{}_{}", uuid::Uuid::new_v4(), clean_filename);
    let date_dir = Utc::now().format("%Y/%m/%d").to_string();
    let save_path = format!("{}/{}", date_dir, uuid_name);

    // 合并分片
    state
        .storage
        .merge_chunks(&upload_id, total_chunks, &save_path)
        .await?;

    let settings = state.config.get();
    let code = generate_code(&settings.code_generate_type);

    let now = Utc::now();
    let (expired_at, expired_count) = match expire_style.as_str() {
        "minute" => (
            Some(now + chrono::Duration::minutes(expire_value as i64)),
            -1,
        ),
        "hour" => (
            Some(now + chrono::Duration::hours(expire_value as i64)),
            -1,
        ),
        "day" => (
            Some(now + chrono::Duration::days(expire_value as i64)),
            -1,
        ),
        "count" => (None, expire_value),
        "forever" => (None, -1),
        _ => (Some(now + chrono::Duration::days(1)), -1),
    };

    let new_file = NewFileCode {
        code: code.clone(),
        prefix: date_dir.clone(),
        suffix: clean_filename.clone(),
        uuid_file_name: Some(uuid_name),
        file_path: Some(date_dir),
        size: file_size,
        text: None,
        expired_at,
        expired_count,
        used_count: 0,
        file_hash: None,
        is_chunked: true,
        upload_id: Some(upload_id.clone()),
    };

    insert_file_code(&state.pool, &new_file).await?;

    // 释放预留并清理
    let _ = release_storage(&state.pool, &upload_id).await;
    let _ = delete_upload_chunks(&state.pool, &upload_id).await;

    Ok(Json(APIResponse::success(json!({
        "code": code,
        "name": clean_filename,
        "size": file_size
    }))))
}

pub async fn chunk_status_handler(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let session_row = get_chunk_session(&state.pool, &upload_id).await?;

    let (file_name, file_size, chunk_size, total_chunks) =
        session_row.ok_or_else(|| AppError::NotFound("上传会话不存在".to_string()))?;

    let uploaded = get_uploaded_chunk_indexes(&state.pool, &upload_id).await?;
    let progress = if total_chunks > 0 {
        (uploaded.len() as f64 / total_chunks as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(APIResponse::success(json!({
        "upload_id": upload_id,
        "file_name": file_name,
        "file_size": file_size,
        "chunk_size": chunk_size,
        "total_chunks": total_chunks,
        "uploaded_chunks": uploaded,
        "progress": progress
    }))))
}

pub async fn chunk_abort_handler(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let _ = state.storage.delete_chunks(&upload_id).await;
    let _ = release_storage(&state.pool, &upload_id).await;
    let _ = delete_upload_chunks(&state.pool, &upload_id).await;

    Ok(Json(APIResponse::success(json!({ "status": "aborted" }))))
}
