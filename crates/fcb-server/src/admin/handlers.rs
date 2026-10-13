use axum::{
    extract::{Query, State},
    response::Json,
};
use chrono::{DateTime, Duration, Utc};
use fcb_core::{
    version::APP_VERSION,
    APIResponse, AppError,
};
use fcb_db::crud::{
    count_total_files, delete_file_code, delete_view_preset, get_file_code_by_code,
    get_file_code_by_id, get_view_presets, insert_file_code, list_all_files, save_view_preset,
    set_key_value, sum_total_storage_size, sum_total_used_count, update_file_policy_download_limit,
    update_file_policy_expired_at, update_file_policy_make_permanent, NewFileCode,
};
use fcb_db::FileCode;
use fcb_security::{
    create_jwt_token, hash_password, is_password_hashed, validate_outbound_endpoint,
    validate_outbound_hostname, verify_jwt_token, verify_password,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

use crate::admin::auth::AdminAuth;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub password: String,
}

pub async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginPayload>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let settings = state.config.get();

    if !verify_password(&payload.password, &settings.admin_token) {
        return Err(AppError::Unauthorized("密码错误".to_string()));
    }

    let expires_in = settings.admin_session_expire;
    let token = create_jwt_token(&settings.jwt_secret, expires_in)?;
    let claims = verify_jwt_token(&token, &settings.jwt_secret)?;

    Ok(Json(APIResponse::success(json!({
        "id": "admin",
        "username": "admin",
        "token": token,
        "token_type": "Bearer",
        "expires_at": claims.exp,
        "expires_in": expires_in
    }))))
}

pub async fn verify_handler(
    auth: AdminAuth,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    Ok(Json(APIResponse::success(json!({
        "id": "admin",
        "username": "admin",
        "token": auth.token,
        "token_type": "Bearer",
        "expires_at": auth.claims.exp,
        "is_admin": true
    }))))
}

pub async fn logout_handler(
    _auth: AdminAuth,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    Ok(Json(APIResponse::success(json!({ "ok": true }))))
}

pub async fn dashboard_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let settings = state.config.get();
    let now = Utc::now();

    let total_files = count_total_files(&state.pool).await?;
    let all_size = sum_total_storage_size(&state.pool).await?;
    let used_count = sum_total_used_count(&state.pool).await?;

    let all_records = list_all_files(&state.pool).await?;
    let text_count = all_records.iter().filter(|f| f.text.is_some()).count() as i64;
    let chunked_count = all_records.iter().filter(|f| f.is_chunked).count() as i64;

    let mut expired_count = 0;
    let recent_files: Vec<serde_json::Value> = all_records
        .iter()
        .take(10)
        .map(|f| {
            let is_exp = f.is_expired(now);
            if is_exp {
                expired_count += 1;
            }
            json!({
                "id": f.id,
                "code": f.code,
                "name": format!("{}{}", f.prefix, f.suffix),
                "suffix": f.suffix,
                "size": f.size,
                "text": f.text.is_some(),
                "expired_at": f.expired_at,
                "expired_count": f.expired_count,
                "used_count": f.used_count,
                "created_at": f.created_at,
                "is_expired": is_exp
            })
        })
        .collect();

    let storage_used_str = all_size.to_string();

    Ok(Json(APIResponse::success(json!({
        "total_files": total_files,
        "storage_used": storage_used_str,
        "yesterday_count": 0,
        "yesterday_size": "0",
        "today_count": total_files,
        "today_size": storage_used_str,
        "sys_uptime": 0,
        "active_count": (total_files - expired_count).max(0),
        "expired_count": expired_count,
        "text_count": text_count,
        "file_count": (total_files - text_count).max(0),
        "chunked_count": chunked_count,
        "used_count": used_count,
        "storage_backend": settings.file_storage,
        "upload_size_limit": settings.upload_size,
        "open_upload": settings.open_upload,
        "enable_chunk": settings.enable_chunk,
        "max_save_seconds": 0,
        "recent_files": recent_files,
        "recent_activities": [],
        "top_suffixes": [],
        "version": APP_VERSION
    }))))
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct FileListQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    pub size: Option<i64>,
    pub page_size: Option<i64>,
    pub keyword: Option<String>,
    pub code: Option<String>,
    pub search: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "type")]
    pub file_type: Option<String>,
    pub health: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

fn default_page() -> i64 {
    1
}

pub fn build_admin_file_item(f: &FileCode, now: DateTime<Utc>) -> serde_json::Value {
    let is_text = f.text.is_some();
    let is_expired = f.is_expired(now);
    let has_download_limit = f.expired_count >= 0;
    let is_permanent = f.expired_at.is_none() && f.expired_count < 0;
    let remaining_downloads = if f.expired_count >= 0 {
        Some(f.expired_count.max(0))
    } else {
        None
    };

    let seconds_until_expiration = f.expired_at.map(|exp| (exp.signed_duration_since(now)).num_seconds());
    let age_seconds = f
        .created_at
        .map(|created| (now.signed_duration_since(created)).num_seconds())
        .unwrap_or(0);

    let mut reasons = Vec::new();
    if is_expired {
        reasons.push("expired");
    }
    if has_download_limit && remaining_downloads == Some(0) {
        reasons.push("download_limit_exhausted");
    }
    if let Some(secs) = seconds_until_expiration {
        if secs > 0 && secs <= 86400 {
            reasons.push("expires_soon");
        }
    }
    if f.used_count == 0 {
        reasons.push("never_retrieved");
    }
    if f.is_chunked {
        reasons.push("chunked_upload");
    }

    let (severity, state, next_action) =
        if is_expired || (has_download_limit && remaining_downloads == Some(0)) {
            ("danger", "expired", "extend_or_delete")
        } else if reasons.contains(&"expires_soon") {
            ("warning", "expiring_soon", "extend_expiration")
        } else if is_permanent {
            ("success", "permanent", "monitor")
        } else {
            ("success", "available", "monitor")
        };

    let name = format!("{}{}", f.prefix, f.suffix);

    json!({
        "id": f.id,
        "code": f.code,
        "name": name,
        "filename": name,
        "display_name": name,
        "prefix": f.prefix,
        "suffix": f.suffix,
        "size": f.size,
        "text": f.text,
        "is_text": is_text,
        "is_expired": is_expired,
        "is_chunked": f.is_chunked,
        "is_permanent": is_permanent,
        "has_download_limit": has_download_limit,
        "has_expiration_time": f.expired_at.is_some(),
        "can_download": true,
        "can_preview_text": is_text,
        "text_length": f.text.as_ref().map(|t| t.len()).unwrap_or(0),
        "expired_at": f.expired_at,
        "expired_count": f.expired_count,
        "remaining_downloads": remaining_downloads,
        "used_count": f.used_count,
        "created_at": f.created_at,
        "file_hash": f.file_hash,
        "upload_id": f.upload_id,
        "type": if is_text { "text" } else { "file" },
        "status": if is_expired { "expired" } else { "active" },
        "status_insights": {
            "severity": severity,
            "state": state,
            "next_action": next_action,
            "reasons": reasons,
            "metrics": {
                "age_seconds": age_seconds.max(0),
                "seconds_until_expiration": seconds_until_expiration,
                "remaining_downloads": remaining_downloads,
                "used_count": f.used_count
            }
        },
        "policy": {
            "expired_at": f.expired_at,
            "expired_count": f.expired_count,
            "remaining_downloads": remaining_downloads,
            "is_expired": is_expired,
            "is_permanent": is_permanent
        },
        "storage": {
            "file_path": f.file_path,
            "uuid_file_name": f.uuid_file_name,
            "file_hash": f.file_hash,
            "is_chunked": f.is_chunked,
            "upload_id": f.upload_id
        }
    })
}

pub async fn file_list_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<FileListQuery>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let page = query.page.max(1);
    let size = query.size.or(query.page_size).unwrap_or(10).clamp(1, 100);

    let all_files = list_all_files(&state.pool).await?;

    let now = Utc::now();
    let kw = query
        .keyword
        .or(query.search)
        .or(query.code)
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let status_filter = query.status.unwrap_or_default().trim().to_lowercase();
    let type_filter = query.file_type.unwrap_or_default().trim().to_lowercase();
    let health_filter = query.health.unwrap_or_default().trim().to_lowercase();

    let total_files = all_files.len();
    let mut active_count = 0;
    let mut expired_count = 0;
    let mut text_count = 0;
    let mut file_count = 0;
    let mut chunked_count = 0;
    let mut health_attention_count = 0;
    let mut health_danger_count = 0;
    let mut health_warning_count = 0;
    let mut expiring_soon_count = 0;
    let mut never_retrieved_count = 0;
    let mut healthy_count = 0;
    let mut permanent_count = 0;
    let mut storage_used = 0i64;
    let mut total_used = 0i64;

    let mut filtered_items = Vec::new();

    for f in all_files {
        let item = build_admin_file_item(&f, now);
        let is_exp = item["is_expired"].as_bool().unwrap_or(false);
        let is_txt = item["is_text"].as_bool().unwrap_or(false);
        let is_chunk = item["is_chunked"].as_bool().unwrap_or(false);
        let is_perm = item["is_permanent"].as_bool().unwrap_or(false);
        let sev = item["status_insights"]["severity"]
            .as_str()
            .unwrap_or("success");
        let reasons = item["status_insights"]["reasons"].as_array();

        if is_exp {
            expired_count += 1;
        } else {
            active_count += 1;
        }
        if is_txt {
            text_count += 1;
        } else {
            file_count += 1;
        }
        if is_chunk {
            chunked_count += 1;
        }
        if is_perm {
            permanent_count += 1;
        }
        if sev == "danger" || sev == "warning" {
            health_attention_count += 1;
        }
        if sev == "danger" {
            health_danger_count += 1;
        }
        if sev == "warning" {
            health_warning_count += 1;
        }
        if sev == "success" {
            healthy_count += 1;
        }

        let has_reason = |r: &str| {
            reasons
                .map(|arr| arr.iter().any(|v| v.as_str() == Some(r)))
                .unwrap_or(false)
        };
        if has_reason("expires_soon") {
            expiring_soon_count += 1;
        }
        if has_reason("never_retrieved") {
            never_retrieved_count += 1;
        }

        storage_used += f.size;
        total_used += f.used_count as i64;

        // 过滤
        if !kw.is_empty() {
            let name_lower = item["name"].as_str().unwrap_or("").to_lowercase();
            let code_lower = f.code.to_lowercase();
            let text_lower = f.text.as_deref().unwrap_or("").to_lowercase();
            if !name_lower.contains(&kw) && !code_lower.contains(&kw) && !text_lower.contains(&kw) {
                continue;
            }
        }
        if !status_filter.is_empty() && status_filter != "all" {
            let st = item["status"].as_str().unwrap_or("");
            if st != status_filter {
                continue;
            }
        }
        if !type_filter.is_empty() && type_filter != "all" {
            let tp = item["type"].as_str().unwrap_or("");
            if tp != type_filter {
                continue;
            }
        }
        if !health_filter.is_empty() && health_filter != "all" {
            let match_health = match health_filter.as_str() {
                "attention" => sev == "danger" || sev == "warning",
                "danger" => sev == "danger",
                "warning" => sev == "warning",
                "expired" => is_exp,
                "expiring_soon" => has_reason("expires_soon"),
                "never_retrieved" => has_reason("never_retrieved"),
                "healthy" => sev == "success",
                "permanent" => is_perm,
                _ => true,
            };
            if !match_health {
                continue;
            }
        }

        filtered_items.push(item);
    }

    let total = filtered_items.len();
    let offset = ((page - 1) * size) as usize;
    let paged_items: Vec<serde_json::Value> = filtered_items
        .into_iter()
        .skip(offset)
        .take(size as usize)
        .collect();

    let summary = json!({
        "total_files": total_files,
        "active_count": active_count,
        "expired_count": expired_count,
        "text_count": text_count,
        "file_count": file_count,
        "chunked_count": chunked_count,
        "health_attention_count": health_attention_count,
        "health_danger_count": health_danger_count,
        "health_warning_count": health_warning_count,
        "expiring_soon_count": expiring_soon_count,
        "storage_issue_count": 0,
        "never_retrieved_count": never_retrieved_count,
        "healthy_count": healthy_count,
        "permanent_count": permanent_count,
        "storage_used": storage_used,
        "used_count": total_used
    });

    Ok(Json(APIResponse::success(json!({
        "page": page,
        "size": size,
        "data": paged_items.clone(),
        "list": paged_items,
        "total": total,
        "summary": summary
    }))))
}

#[derive(Debug, Deserialize, Default)]
pub struct FileDetailQuery {
    pub id: Option<i64>,
}

pub async fn file_detail_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<FileDetailQuery>,
    body_bytes: axum::body::Bytes,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let mut file_id = query.id;
    if file_id.is_none() && !body_bytes.is_empty() {
        if let Ok(p) = serde_json::from_slice::<FileDetailQuery>(&body_bytes) {
            file_id = p.id;
        }
    }
    let id = file_id.ok_or_else(|| AppError::BadRequest("缺少 id 参数".to_string()))?;

    let f = get_file_code_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("文件不存在".to_string()))?;

    let now = Utc::now();
    let mut detail = build_admin_file_item(&f, now);

    let timeline = json!([
        {
            "key": "created",
            "title": "文件创建",
            "time": f.created_at,
            "status": "success",
            "value": f.created_at
        },
        {
            "key": "retrieved",
            "title": "提取次数",
            "status": "info",
            "value": f.used_count
        }
    ]);
    if let Some(obj) = detail.as_object_mut() {
        obj.insert("timeline".to_string(), timeline);
    }

    Ok(Json(APIResponse::success(detail)))
}

#[derive(Debug, Deserialize)]
pub struct FilePolicyActionPayload {
    pub id: i64,
    pub action: String,
    pub download_limit: Option<i32>,
}

pub async fn file_policy_action_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(payload): Json<FilePolicyActionPayload>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let f = get_file_code_by_id(&state.pool, payload.id)
        .await?
        .ok_or_else(|| AppError::NotFound("文件不存在".to_string()))?;

    let now = Utc::now();

    match payload.action.as_str() {
        "extend_24h" => {
            let base = f.expired_at.unwrap_or(now).max(now);
            let next_exp = base + Duration::hours(24);
            update_file_policy_expired_at(&state.pool, f.id, Some(next_exp)).await?;
        }
        "extend_7d" => {
            let base = f.expired_at.unwrap_or(now).max(now);
            let next_exp = base + Duration::days(7);
            update_file_policy_expired_at(&state.pool, f.id, Some(next_exp)).await?;
        }
        "make_permanent" => {
            update_file_policy_make_permanent(&state.pool, f.id).await?;
        }
        "reset_download_limit" => {
            let limit = payload.download_limit.unwrap_or(5);
            update_file_policy_download_limit(&state.pool, f.id, limit).await?;
        }
        _ => return Err(AppError::BadRequest("不支持的策略动作".to_string())),
    }

    let updated = get_file_code_by_id(&state.pool, payload.id)
        .await?
        .ok_or_else(|| AppError::NotFound("文件不存在".to_string()))?;

    Ok(Json(APIResponse::success(build_admin_file_item(&updated, Utc::now()))))
}

#[derive(Debug, Deserialize)]
pub struct BatchPolicyActionPayload {
    pub ids: Vec<i64>,
    pub action: String,
    pub download_limit: Option<i32>,
}

pub async fn file_batch_policy_action_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(payload): Json<BatchPolicyActionPayload>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let now = Utc::now();
    for id in &payload.ids {
        if let Ok(Some(f)) = get_file_code_by_id(&state.pool, *id).await {
            match payload.action.as_str() {
                "extend_24h" => {
                    let base = f.expired_at.unwrap_or(now).max(now);
                    let _ = update_file_policy_expired_at(&state.pool, f.id, Some(base + Duration::hours(24))).await;
                }
                "extend_7d" => {
                    let base = f.expired_at.unwrap_or(now).max(now);
                    let _ = update_file_policy_expired_at(&state.pool, f.id, Some(base + Duration::days(7))).await;
                }
                "make_permanent" => {
                    let _ = update_file_policy_make_permanent(&state.pool, f.id).await;
                }
                "reset_download_limit" => {
                    let limit = payload.download_limit.unwrap_or(5);
                    let _ = update_file_policy_download_limit(&state.pool, f.id, limit).await;
                }
                _ => {}
            }
        }
    }
    Ok(Json(APIResponse::success(json!({
        "updated": payload.ids.len()
    }))))
}

#[derive(Debug, Deserialize)]
pub struct FileMetadataPayload {
    pub id: i64,
    pub note: Option<String>,
    pub tags: Option<Vec<String>>,
}

pub async fn file_metadata_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(payload): Json<FileMetadataPayload>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let meta_key = format!("meta_{}", payload.id);
    let meta_json = json!({
        "note": payload.note.unwrap_or_default(),
        "tags": payload.tags.unwrap_or_default()
    });

    let _ = set_key_value(&state.pool, &meta_key, &serde_json::to_string(&meta_json).unwrap_or_default()).await;

    Ok(Json(APIResponse::success(meta_json)))
}

#[derive(Debug, Deserialize)]
pub struct DeleteFileQuery {
    pub id: Option<i64>,
    pub code: Option<String>,
}

pub async fn delete_file_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<DeleteFileQuery>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let file = if let Some(id) = query.id {
        get_file_code_by_id(&state.pool, id).await?
    } else if let Some(code) = query.code {
        get_file_code_by_code(&state.pool, &code).await?
    } else {
        return Err(AppError::BadRequest("缺少 id 或 code 参数".to_string()));
    };

    let item = file.ok_or_else(|| AppError::NotFound("文件记录不存在".to_string()))?;

    // 删除存储介质中的物理文件
    if item.file_path.as_deref() != Some("local-ref") {
        let full_path = item.get_file_path();
        if !full_path.is_empty() {
            let _ = state.storage.delete_file(&full_path).await;
        }
    }

    delete_file_code(&state.pool, item.id).await?;

    Ok(Json(APIResponse::success(json!({ "ok": true }))))
}

#[derive(Debug, Deserialize)]
pub struct BatchDeletePayload {
    #[serde(default)]
    pub ids: Vec<i64>,
    #[serde(default)]
    pub codes: Vec<String>,
}

pub async fn batch_delete_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(payload): Json<BatchDeletePayload>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    for id in payload.ids {
        if let Ok(Some(f)) = get_file_code_by_id(&state.pool, id).await {
            if f.file_path.as_deref() != Some("local-ref") {
                let full = f.get_file_path();
                if !full.is_empty() {
                    let _ = state.storage.delete_file(&full).await;
                }
            }
            let _ = delete_file_code(&state.pool, id).await;
        }
    }

    for code in payload.codes {
        if let Ok(Some(f)) = get_file_code_by_code(&state.pool, &code).await {
            if f.file_path.as_deref() != Some("local-ref") {
                let full = f.get_file_path();
                if !full.is_empty() {
                    let _ = state.storage.delete_file(&full).await;
                }
            }
            let _ = delete_file_code(&state.pool, f.id).await;
        }
    }

    Ok(Json(APIResponse::success(json!({ "ok": true }))))
}

pub async fn config_get_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let settings = state.config.get();
    let val = serde_json::to_value(&*settings).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(APIResponse::success(val)))
}

pub async fn config_update_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(mut payload): Json<Value>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let is_prod = std::env::var("APP_ENV").unwrap_or_default() == "production";

    if let Some(s3_url) = payload.get("s3_endpoint_url").and_then(|v| v.as_str()) {
        validate_outbound_endpoint(s3_url, is_prod)?;
    }
    if let Some(dav_url) = payload.get("webdav_url").and_then(|v| v.as_str()) {
        validate_outbound_endpoint(dav_url, is_prod)?;
    }
    if let Some(s3_host) = payload.get("s3_hostname").and_then(|v| v.as_str()) {
        validate_outbound_hostname(s3_host, is_prod)?;
    }

    // 若更新密码，且未哈希，自动哈希
    let raw_pass = payload.get("admin_token").and_then(|v| v.as_str()).map(|s| s.to_string());
    if let Some(new_pass) = raw_pass {
        if !new_pass.is_empty() && !is_password_hashed(&new_pass) {
            if let Value::Object(ref mut map) = payload {
                map.insert("admin_token".to_string(), json!(hash_password(&new_pass)));
            }
        }
    }

    state.config.update(payload).await?;
    let updated = state.config.get();

    // 动态热更新底层存储引擎（支持运行时切换为 S3/MinIO、WebDAV 或 Local）
    let new_storage_cfg = crate::state::storage_config_from_settings(&updated);
    if let Err(e) = state.storage.update(&new_storage_cfg) {
        tracing::warn!("Failed to reconfigure storage dynamically: {}", e);
    } else {
        tracing::info!("Storage dynamically reconfigured: {:?}", updated.file_storage);
    }

    let val = serde_json::to_value(&*updated).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(APIResponse::success(val)))
}

fn get_local_root() -> std::path::PathBuf {
    let root = std::path::PathBuf::from("data/local");
    let _ = std::fs::create_dir_all(&root);
    root
}

#[derive(Debug, Deserialize)]
pub struct LocalListQuery {
    #[serde(default)]
    pub path: Option<String>,
}

pub async fn local_lists_handler(
    _auth: AdminAuth,
    Query(query): Query<LocalListQuery>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let local_root = get_local_root();
    let raw_path = query.path.unwrap_or_default();
    let clean_rel = raw_path
        .replace('\\', "/")
        .trim_matches('/')
        .to_string();

    let target_dir = if clean_rel.is_empty() {
        local_root.clone()
    } else {
        local_root.join(&clean_rel)
    };

    if !target_dir.exists() || !target_dir.is_dir() {
        return Err(AppError::NotFound("目录不存在".to_string()));
    }

    let mut entries = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(&target_dir) {
        let mut children: Vec<_> = read_dir.flatten().collect();
        children.sort_by(|a, b| {
            let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            b_is_dir.cmp(&a_is_dir).then_with(|| a.file_name().cmp(&b.file_name()))
        });

        for entry in children {
            let p = entry.path();
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            let is_dir = p.is_dir();
            let size = if is_dir {
                None
            } else {
                p.metadata().ok().map(|m| m.len())
            };
            let child_rel = if clean_rel.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", clean_rel, name)
            };
            let ctime = p
                .metadata()
                .ok()
                .and_then(|m| m.modified().or_else(|_| m.created()).ok())
                .map(|t| {
                    let dt: chrono::DateTime<Utc> = t.into();
                    dt.format("%Y-%m-%d %H:%M:%S").to_string()
                });

            entries.push(json!({
                "file": name,
                "name": name,
                "path": child_rel,
                "type": if is_dir { "dir" } else { "file" },
                "ctime": ctime,
                "size": size
            }));
        }
    }

    let parent = if clean_rel.is_empty() {
        String::new()
    } else {
        Path::new(&clean_rel)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    };

    Ok(Json(APIResponse::success(json!({
        "path": clean_rel,
        "parent": parent,
        "truncated": false,
        "items": entries
    }))))
}

#[derive(Debug, Deserialize, Default)]
pub struct LocalSharePayload {
    pub filename: Option<String>,
    pub path: Option<String>,
    #[serde(default = "default_expire_val")]
    pub expire_value: i32,
    #[serde(default = "default_expire_sty")]
    pub expire_style: String,
}

fn default_expire_val() -> i32 {
    1
}
fn default_expire_sty() -> String {
    "day".to_string()
}

pub async fn local_share_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(payload): Json<LocalSharePayload>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let rel_path = payload
        .filename
        .or(payload.path)
        .unwrap_or_default()
        .replace('\\', "/");
    let clean_rel = rel_path.trim_matches('/').to_string();

    let full_path = get_local_root().join(&clean_rel);
    if !full_path.exists() || !full_path.is_file() {
        return Err(AppError::NotFound("本地文件不存在".to_string()));
    }

    let meta = full_path.metadata().map_err(|e| AppError::Internal(e.to_string()))?;
    let file_size = meta.len() as i64;
    let file_name = full_path.file_name().unwrap_or_default().to_string_lossy().to_string();

    let settings = state.config.get();
    let code = fcb_core::generate_code(&settings.code_generate_type);

    let now = Utc::now();
    let (expired_at, expired_count) = match payload.expire_style.as_str() {
        "minute" => (
            Some(now + chrono::Duration::minutes(payload.expire_value as i64)),
            -1,
        ),
        "hour" => (
            Some(now + chrono::Duration::hours(payload.expire_value as i64)),
            -1,
        ),
        "day" => (
            Some(now + chrono::Duration::days(payload.expire_value as i64)),
            -1,
        ),
        "count" => (None, payload.expire_value),
        "forever" => (None, -1),
        _ => (Some(now + chrono::Duration::days(1)), -1),
    };

    let new_file = NewFileCode {
        code: code.clone(),
        prefix: String::new(),
        suffix: file_name.clone(),
        uuid_file_name: Some(clean_rel),
        file_path: Some("local-ref".to_string()),
        size: file_size,
        text: None,
        expired_at,
        expired_count,
        used_count: 0,
        file_hash: None,
        is_chunked: false,
        upload_id: None,
    };

    let inserted = insert_file_code(&state.pool, &new_file).await?;

    Ok(Json(APIResponse::success(json!({
        "id": inserted.id,
        "code": code,
        "name": file_name,
        "size": file_size
    }))))
}

#[derive(Debug, Deserialize, Default)]
pub struct LocalDeletePayload {
    pub filename: Option<String>,
}

pub async fn local_delete_handler(
    _auth: AdminAuth,
    Query(query): Query<LocalDeletePayload>,
    body_bytes: axum::body::Bytes,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let mut filename = query.filename.unwrap_or_default();
    if filename.is_empty() && !body_bytes.is_empty() {
        if let Ok(val) = serde_json::from_slice::<LocalDeletePayload>(&body_bytes) {
            if let Some(f) = val.filename {
                filename = f;
            }
        }
    }
    let clean_rel = filename.replace('\\', "/").trim_matches('/').to_string();

    let full_path = get_local_root().join(&clean_rel);
    if full_path.exists() {
        if full_path.is_dir() {
            let _ = std::fs::remove_dir_all(&full_path);
        } else {
            let _ = std::fs::remove_file(&full_path);
        }
        Ok(Json(APIResponse::success(json!("删除成功"))))
    } else {
        Err(AppError::NotFound("文件不存在".to_string()))
    }
}

// ============================================================================
// View Presets
// ============================================================================

pub async fn file_view_presets_get_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let presets = get_view_presets(&state.pool).await?;
    let total = presets.len();
    Ok(Json(APIResponse::success(json!({
        "presets": presets,
        "total": total
    }))))
}

#[derive(Debug, Deserialize)]
pub struct FileViewPresetPayload {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub filters: serde_json::Value,
}

pub async fn file_view_presets_save_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(payload): Json<FileViewPresetPayload>,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let updated = save_view_preset(&state.pool, payload.id, payload.name, payload.filters).await?;
    Ok(Json(APIResponse::success(updated)))
}

#[derive(Debug, Deserialize, Default)]
pub struct FileViewPresetDeletePayload {
    pub id: Option<String>,
}

pub async fn file_view_presets_delete_handler(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(query): Query<FileViewPresetDeletePayload>,
    body_bytes: axum::body::Bytes,
) -> Result<Json<APIResponse<serde_json::Value>>, AppError> {
    let mut preset_id = query.id.unwrap_or_default();
    if preset_id.is_empty() && !body_bytes.is_empty() {
        if let Ok(val) = serde_json::from_slice::<FileViewPresetDeletePayload>(&body_bytes) {
            if let Some(id) = val.id {
                preset_id = id;
            }
        }
    }

    let _ = delete_view_preset(&state.pool, &preset_id).await?;
    Ok(Json(APIResponse::success(json!({ "ok": true }))))
}
