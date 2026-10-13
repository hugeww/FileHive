use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use chrono::{Duration, Utc};
use fcb_core::{generate_code, sanitize_filename, AppError};
use fcb_db::crud::{insert_file_code, NewFileCode};
use serde_json::{json, Value};

use crate::state::AppState;

const MCP_PROTOCOL_VERSION: &str = "2025-03-26";
const TEXT_SHARE_MAX_BYTES: usize = 222 * 1024;

fn jsonrpc_error(id: Option<Value>, code: i32, msg: &str) -> Response {
    let body = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": msg
        }
    });
    (StatusCode::BAD_REQUEST, Json(body)).into_response()
}

fn jsonrpc_result(id: Option<Value>, result: Value) -> Response {
    let body = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .header("MCP-Protocol-Version", MCP_PROTOCOL_VERSION)
        .body(axum::body::Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "share_text",
            "description": "将文本快速上传到 FileHive 并创建分享链接。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "要分享的文本" },
                    "expire_value": { "type": "integer", "minimum": 1, "default": 1 },
                    "expire_style": { "type": "string", "enum": ["day", "hour", "minute", "forever", "count"], "default": "day" }
                },
                "required": ["text"]
            }
        },
        {
            "name": "share_file",
            "description": "将 Base64 编码的文件上传到 FileHive 并创建分享链接。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "filename": { "type": "string", "description": "文件名" },
                    "content_base64": { "type": "string", "description": "文件内容的标准 Base64 编码" },
                    "content_type": { "type": "string", "default": "application/octet-stream" },
                    "expire_value": { "type": "integer", "minimum": 1, "default": 1 },
                    "expire_style": { "type": "string", "enum": ["day", "hour", "minute", "forever", "count"], "default": "day" }
                },
                "required": ["filename", "content_base64"]
            }
        }
    ])
}

pub async fn mcp_handler(
    State(state): State<AppState>,
    _headers: HeaderMap,
    Json(msg): Json<Value>,
) -> Response {
    if msg.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
        return jsonrpc_error(msg.get("id").cloned(), -32600, "Invalid Request");
    }

    let method = match msg.get("method").and_then(|v| v.as_str()) {
        Some(m) => m,
        None => return jsonrpc_error(msg.get("id").cloned(), -32600, "Invalid Request"),
    };

    let id = msg.get("id").cloned();

    match method {
        "initialize" => jsonrpc_result(
            id,
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "filehive", "version": "1.0.0" }
            }),
        ),
        "ping" => jsonrpc_result(id, json!({})),
        "tools/list" => jsonrpc_result(id, json!({ "tools": tool_definitions() })),
        "tools/call" => {
            let params = match msg.get("params").and_then(|v| v.as_object()) {
                Some(p) => p,
                None => return jsonrpc_error(id, -32602, "Invalid params"),
            };
            let name = match params.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return jsonrpc_error(id, -32602, "Invalid params"),
            };
            let args = params.get("arguments").cloned().unwrap_or(json!({}));

            match handle_tool_call(&state, name, args).await {
                Ok(val) => {
                    let text = serde_json::to_string(&val).unwrap();
                    jsonrpc_result(
                        id,
                        json!({
                            "content": [{ "type": "text", "text": text }],
                            "structuredContent": val
                        }),
                    )
                }
                Err(err) => {
                    let err_msg = err.message();
                    jsonrpc_result(
                        id,
                        json!({
                            "content": [{ "type": "text", "text": err_msg }],
                            "isError": true
                        }),
                    )
                }
            }
        }
        _ => jsonrpc_error(id, -32601, "Method not found"),
    }
}

async fn handle_tool_call(
    state: &AppState,
    tool_name: &str,
    args: Value,
) -> Result<Value, AppError> {
    let settings = state.config.get();

    if tool_name == "share_text" {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("text 必须是字符串".to_string()))?;

        if text.as_bytes().len() > TEXT_SHARE_MAX_BYTES {
            return Err(AppError::BadRequest("内容过多，请改用文件分享".to_string()));
        }

        let expire_value = args.get("expire_value").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
        let expire_style = args
            .get("expire_style")
            .and_then(|v| v.as_str())
            .unwrap_or("day");

        let code = generate_code(&settings.code_generate_type);
        let now = Utc::now();
        let (expired_at, expired_count) = match expire_style {
            "minute" => (Some(now + Duration::minutes(expire_value as i64)), -1),
            "hour" => (Some(now + Duration::hours(expire_value as i64)), -1),
            "day" => (Some(now + Duration::days(expire_value as i64)), -1),
            "count" => (None, expire_value),
            "forever" => (None, -1),
            _ => (Some(now + Duration::days(1)), -1),
        };

        let new_code = NewFileCode {
            code: code.clone(),
            prefix: String::new(),
            suffix: String::new(),
            uuid_file_name: None,
            file_path: None,
            size: text.as_bytes().len() as i64,
            text: Some(text.to_string()),
            expired_at,
            expired_count,
            used_count: 0,
            file_hash: None,
            is_chunked: false,
            upload_id: None,
        };

        insert_file_code(&state.pool, &new_code).await?;

        let url = format!("/#/?code={}", code);
        Ok(json!({
            "code": code,
            "name": "Text",
            "url": url,
            "markdown": format!("[Text]({})", url)
        }))
    } else if tool_name == "share_file" {
        let filename = args
            .get("filename")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("filename 不能为空".to_string()))?;
        let base64_str = args
            .get("content_base64")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("content_base64 必须是字符串".to_string()))?;

        let bytes = BASE64_STANDARD
            .decode(base64_str)
            .map_err(|_| AppError::BadRequest("content_base64 不是有效的标准 Base64".to_string()))?;

        if bytes.len() as i64 > settings.upload_size {
            return Err(AppError::PayloadTooLarge("文件大小超出限制".to_string()));
        }

        let clean_filename = sanitize_filename(filename);
        let uuid_name = format!("{}_{}", uuid::Uuid::new_v4(), clean_filename);
        let date_dir = Utc::now().format("%Y/%m/%d").to_string();
        let save_path = format!("{}/{}", date_dir, uuid_name);

        state
            .storage
            .save_file(&save_path, bytes::Bytes::from(bytes.clone()), None)
            .await?;

        let expire_value = args.get("expire_value").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
        let expire_style = args
            .get("expire_style")
            .and_then(|v| v.as_str())
            .unwrap_or("day");

        let code = generate_code(&settings.code_generate_type);
        let now = Utc::now();
        let (expired_at, expired_count) = match expire_style {
            "minute" => (Some(now + Duration::minutes(expire_value as i64)), -1),
            "hour" => (Some(now + Duration::hours(expire_value as i64)), -1),
            "day" => (Some(now + Duration::days(expire_value as i64)), -1),
            "count" => (None, expire_value),
            "forever" => (None, -1),
            _ => (Some(now + Duration::days(1)), -1),
        };

        let new_code = NewFileCode {
            code: code.clone(),
            prefix: date_dir.clone(),
            suffix: clean_filename.clone(),
            uuid_file_name: Some(uuid_name),
            file_path: Some(date_dir),
            size: bytes.len() as i64,
            text: None,
            expired_at,
            expired_count,
            used_count: 0,
            file_hash: None,
            is_chunked: false,
            upload_id: None,
        };

        insert_file_code(&state.pool, &new_code).await?;

        let url = format!("/#/?code={}", code);
        Ok(json!({
            "code": code,
            "name": clean_filename,
            "url": url,
            "markdown": format!("[{}]({})", clean_filename, url)
        }))
    } else {
        Err(AppError::BadRequest(format!("未知工具：{}", tool_name)))
    }
}
