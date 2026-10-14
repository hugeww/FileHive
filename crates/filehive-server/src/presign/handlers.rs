use axum::{
    body::Bytes,
    extract::{Multipart, Path, State},
    response::Json,
};
use chrono::{Duration, Utc};
use filehive_config::quota::{release_storage, reserve_storage};
use filehive_core::{sanitize_filename, APIResponse, AppError};
use filehive_db::crud::{
    delete_presign_session, get_presign_session, insert_file_code, insert_presign_session,
    NewFileCode,
};
use serde::Deserialize;
use serde_json::json;

use crate::state::AppState;

const PRESIGN_SESSION_EXPIRES: i64 = 1800; // 30 minutes

#[derive(Debug, Deserialize)]
pub struct PresignInitRequest {
    pub file_name: String,
    pub file_size: i64,
    #[serde(default = "default_expire_value")]
    pub expire_value: i32,
    #[serde(default = "default_expire_style")]
    pub expire_style: String,
}

fn default_expire_value() -> i32 {
    1
}
fn default_expire_style() -> String {
    "day".to_string()
}

pub async fn presign_init_handler(
    State(state): State<AppState>,
    Json(payload): Json<PresignInitRequest>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let settings = state.config.get();
    if payload.file_size > settings.upload_size {
        return Err(AppError::PayloadTooLarge(format!(
            "文件大小超过限制，最大为 {} MB",
            settings.upload_size / (1024 * 1024)
        )));
    }

    let upload_id = uuid::Uuid::new_v4().simple().to_string();
    let reservation_token = format!("presign:{}", upload_id);

    reserve_storage(
        &state.pool,
        &reservation_token,
        payload.file_size,
        PRESIGN_SESSION_EXPIRES,
    )
    .await?;

    let clean_name = sanitize_filename(&payload.file_name);
    let date_dir = Utc::now().format("%Y/%m/%d").to_string();
    let uuid_name = format!("{}_{}", upload_id, clean_name);
    let save_path = format!("{}/{}", date_dir, uuid_name);

    let expires_at = Utc::now() + Duration::seconds(PRESIGN_SESSION_EXPIRES);

    // 在本地或无直传时使用 proxy 模式
    let mode = "proxy".to_string();
    let proxy_upload_url = format!("/presign/upload/proxy/{}", upload_id);

    insert_presign_session(
        &state.pool,
        &upload_id,
        &clean_name,
        payload.file_size,
        &save_path,
        &mode,
        payload.expire_value,
        &payload.expire_style,
        expires_at,
    )
    .await?;

    Ok(Json(APIResponse::success(json!({
        "upload_id": upload_id,
        "upload_url": proxy_upload_url,
        "mode": mode,
        "expires_in": PRESIGN_SESSION_EXPIRES,
        "proxy_upload_url": proxy_upload_url
    }))))
}

pub async fn presign_proxy_upload_handler(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let session = get_presign_session(&state.pool, &upload_id)
        .await?
        .ok_or_else(|| AppError::NotFound("上传会话不存在".to_string()))?;

    if session.is_expired(Utc::now()) {
        return Err(AppError::BadRequest("上传会话已过期".to_string()));
    }
    if session.mode != "proxy" {
        return Err(AppError::BadRequest("此会话不支持proxy模式".to_string()));
    }

    let mut found_bytes: Option<Bytes> = None;
    let mut found_name = session.file_name.clone();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        if field.name() == Some("file") {
            if let Some(f_name) = field.file_name() {
                found_name = sanitize_filename(f_name);
            }
            found_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
            break;
        }
    }

    let file_data = found_bytes.ok_or_else(|| AppError::BadRequest("未接收到上传文件".to_string()))?;
    let size = file_data.len() as i64;

    // 保存文件到存储后端
    state
        .storage
        .save_file(&session.save_path, file_data, None)
        .await?;

    let settings = state.config.get();
    let code = filehive_core::generate_code(&settings.code_generate_type);

    let now = Utc::now();
    let (expired_at, expired_count) = match session.expire_style.as_str() {
        "minute" => (
            Some(now + Duration::minutes(session.expire_value as i64)),
            -1,
        ),
        "hour" => (
            Some(now + Duration::hours(session.expire_value as i64)),
            -1,
        ),
        "day" => (
            Some(now + Duration::days(session.expire_value as i64)),
            -1,
        ),
        "count" => (None, session.expire_value),
        "forever" => (None, -1),
        _ => (Some(now + Duration::days(1)), -1),
    };

    let dir_path = std::path::Path::new(&session.save_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let uuid_name = std::path::Path::new(&session.save_path)
        .file_name()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let new_file = NewFileCode {
        code: code.clone(),
        prefix: dir_path.clone(),
        suffix: found_name.clone(),
        uuid_file_name: Some(uuid_name),
        file_path: Some(dir_path),
        size,
        text: None,
        expired_at,
        expired_count,
        used_count: 0,
        file_hash: None,
        is_chunked: false,
        upload_id: Some(upload_id.clone()),
    };

    insert_file_code(&state.pool, &new_file).await?;

    // 删除会话与释放容量预留
    let reservation_token = format!("presign:{}", upload_id);
    let _ = release_storage(&state.pool, &reservation_token).await;
    let _ = delete_presign_session(&state.pool, &upload_id).await;

    Ok(Json(APIResponse::success(json!({
        "code": code,
        "name": session.file_name
    }))))
}

pub async fn presign_confirm_handler(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let session = get_presign_session(&state.pool, &upload_id)
        .await?
        .ok_or_else(|| AppError::NotFound("上传会话不存在".to_string()))?;

    if session.is_expired(Utc::now()) {
        return Err(AppError::BadRequest("上传会话已过期".to_string()));
    }

    let settings = state.config.get();
    let code = filehive_core::generate_code(&settings.code_generate_type);

    let now = Utc::now();
    let (expired_at, expired_count) = match session.expire_style.as_str() {
        "minute" => (
            Some(now + Duration::minutes(session.expire_value as i64)),
            -1,
        ),
        "hour" => (
            Some(now + Duration::hours(session.expire_value as i64)),
            -1,
        ),
        "day" => (
            Some(now + Duration::days(session.expire_value as i64)),
            -1,
        ),
        "count" => (None, session.expire_value),
        "forever" => (None, -1),
        _ => (Some(now + Duration::days(1)), -1),
    };

    let dir_path = std::path::Path::new(&session.save_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let uuid_name = std::path::Path::new(&session.save_path)
        .file_name()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let new_file = NewFileCode {
        code: code.clone(),
        prefix: dir_path.clone(),
        suffix: session.file_name.clone(),
        uuid_file_name: Some(uuid_name),
        file_path: Some(dir_path),
        size: session.file_size,
        text: None,
        expired_at,
        expired_count,
        used_count: 0,
        file_hash: None,
        is_chunked: false,
        upload_id: Some(upload_id.clone()),
    };

    insert_file_code(&state.pool, &new_file).await?;

    let reservation_token = format!("presign:{}", upload_id);
    let _ = release_storage(&state.pool, &reservation_token).await;
    let _ = delete_presign_session(&state.pool, &upload_id).await;

    Ok(Json(APIResponse::success(json!({
        "code": code,
        "name": session.file_name
    }))))
}

pub async fn presign_status_handler(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let session = get_presign_session(&state.pool, &upload_id)
        .await?
        .ok_or_else(|| AppError::NotFound("上传会话不存在".to_string()))?;

    let is_exp = session.is_expired(Utc::now());

    Ok(Json(APIResponse::success(json!({
        "upload_id": session.upload_id,
        "file_name": session.file_name,
        "file_size": session.file_size,
        "mode": session.mode,
        "created_at": session.created_at,
        "expires_at": session.expires_at,
        "is_expired": is_exp
    }))))
}

pub async fn presign_cancel_handler(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let session = get_presign_session(&state.pool, &upload_id)
        .await?
        .ok_or_else(|| AppError::NotFound("上传会话不存在".to_string()))?;

    let reservation_token = format!("presign:{}", upload_id);
    let _ = release_storage(&state.pool, &reservation_token).await;

    let _ = state.storage.delete_file(&session.save_path).await;
    let _ = delete_presign_session(&state.pool, &upload_id).await;

    Ok(Json(APIResponse::success(json!({ "ok": true }))))
}
