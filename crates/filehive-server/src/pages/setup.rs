use std::collections::HashMap;
use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json, Redirect, Response},
};
use filehive_core::{APIResponse, AppError};
use filehive_security::{generate_jwt_secret, hash_password};
use serde::Deserialize;
use serde_json::json;

use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct SetupPayload {
    #[serde(default)]
    pub admin_password: Option<String>,
    #[serde(default)]
    pub confirm_password: Option<String>,
    #[serde(default)]
    pub site_name: Option<String>,
    #[serde(default)]
    pub expire_style: Option<Vec<String>>,
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

pub fn render_setup_page(error: Option<&str>, site_name: &str) -> String {
    let error_html = if let Some(err) = error {
        format!(
            r#"<div style="background: #fee2e2; border: 1px solid #ef4444; color: #b91c1c; padding: 12px; border-radius: 6px; margin-bottom: 20px; font-size: 14px;">{}</div>"#,
            escape_html(err)
        )
    } else {
        String::new()
    };

    let safe_site_name = escape_html(if site_name.is_empty() {
        "文件蜂巢 - FileHive"
    } else {
        site_name
    });

    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <title>FileHive - 初始化向导</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #f0f2f5; display: flex; justify-content: center; align-items: center; min-height: 100vh; margin: 0; }}
        .card {{ background: white; padding: 32px; border-radius: 12px; box-shadow: 0 4px 16px rgba(0,0,0,0.08); width: 100%; max-width: 440px; }}
        h2 {{ margin-top: 0; color: #1f2937; text-align: center; }}
        .form-group {{ margin-bottom: 20px; }}
        label {{ display: block; margin-bottom: 6px; font-weight: 500; color: #4b5563; }}
        input[type="text"], input[type="password"] {{ width: 100%; padding: 10px 12px; border: 1px solid #d1d5db; border-radius: 6px; box-sizing: border-box; font-size: 14px; }}
        button {{ width: 100%; padding: 12px; background: #3b82f6; color: white; border: none; border-radius: 6px; font-size: 16px; font-weight: 600; cursor: pointer; }}
        button:hover {{ background: #2563eb; }}
    </style>
</head>
<body>
    <div class="card">
        <h2>FileHive 系统初始化</h2>
        {}
        <form method="POST" action="/setup">
            <div class="form-group">
                <label>站点名称</label>
                <input type="text" name="site_name" value="{}" required />
            </div>
            <div class="form-group">
                <label>管理员密码 (至少 8 位)</label>
                <input type="password" name="admin_password" required minlength="8" />
            </div>
            <div class="form-group">
                <label>确认管理员密码</label>
                <input type="password" name="confirm_password" required minlength="8" />
            </div>
            <button type="submit">完成设置</button>
        </form>
    </div>
</body>
</html>"#,
        error_html, safe_site_name
    )
}

fn respond_error(accepts_html: bool, status: StatusCode, msg: &str, site_name: &str) -> Response {
    if accepts_html {
        (status, Html(render_setup_page(Some(msg), site_name))).into_response()
    } else {
        (
            status,
            Json(json!({
                "code": status.as_u16(),
                "message": msg,
                "msg": msg,
                "detail": msg
            })),
        )
            .into_response()
    }
}

pub async fn setup_get_handler(State(state): State<AppState>) -> Response {
    let settings = state.config.get();
    if !settings.admin_token.is_empty() {
        return Redirect::to("/").into_response();
    }

    Html(render_setup_page(None, &settings.name)).into_response()
}

pub async fn setup_post_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AppError> {
    let settings = state.config.get();
    if !settings.admin_token.is_empty() {
        return Ok(Redirect::to("/").into_response());
    }

    let accept = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let accepts_html = accept.contains("text/html");

    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let payload: SetupPayload = if content_type.contains("application/json") {
        match serde_json::from_slice::<SetupPayload>(&body) {
            Ok(p) => p,
            Err(e) => {
                return Ok(respond_error(
                    accepts_html,
                    StatusCode::BAD_REQUEST,
                    &format!("JSON 格式解析失败: {}", e),
                    "",
                ));
            }
        }
    } else {
        // Form urlencoded
        let mut map: HashMap<String, String> = HashMap::new();
        let mut expire_styles = Vec::new();

        for (k, v) in url::form_urlencoded::parse(&body) {
            if k == "expire_style" {
                expire_styles.push(v.to_string());
            } else {
                map.insert(k.to_string(), v.to_string());
            }
        }

        SetupPayload {
            admin_password: map.remove("admin_password"),
            confirm_password: map.remove("confirm_password"),
            site_name: map.remove("site_name"),
            expire_style: if expire_styles.is_empty() {
                None
            } else {
                Some(expire_styles)
            },
        }
    };

    let site_name = payload
        .site_name
        .unwrap_or_else(|| "文件蜂巢 - FileHive".to_string());

    let pass = payload.admin_password.unwrap_or_default().trim().to_string();
    let confirm = payload.confirm_password.unwrap_or_default().trim().to_string();

    if pass.len() < 8 {
        return Ok(respond_error(
            accepts_html,
            StatusCode::BAD_REQUEST,
            "管理员密码长度至少为 8 位",
            &site_name,
        ));
    }
    if pass != confirm {
        return Ok(respond_error(
            accepts_html,
            StatusCode::BAD_REQUEST,
            "两次输入的管理员密码不一致",
            &site_name,
        ));
    }

    let hashed_pass = hash_password(&pass);
    let jwt_secret = generate_jwt_secret();

    let mut update_json = json!({
        "admin_token": hashed_pass,
        "jwt_secret": jwt_secret,
        "name": site_name,
        "show_admin_addr": 1
    });

    if let Some(styles) = payload.expire_style {
        if let Some(obj) = update_json.as_object_mut() {
            obj.insert("expire_style".to_string(), json!(styles));
        }
    }

    state.config.update(update_json).await?;

    if accept.contains("application/json") {
        Ok(Json(APIResponse::success(json!({
            "ok": true,
            "admin": "/#/admin"
        })))
        .into_response())
    } else {
        let success_html = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <title>初始化完成 - FileHive</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #f0f2f5; display: flex; justify-content: center; align-items: center; min-height: 100vh; margin: 0; }
        .card { background: white; padding: 36px 32px; border-radius: 12px; box-shadow: 0 4px 16px rgba(0,0,0,0.08); width: 100%; max-width: 440px; text-align: center; }
        h2 { margin-top: 0; color: #1f2937; font-size: 22px; }
        p { color: #4b5563; font-size: 15px; line-height: 1.6; margin: 16px 0 28px; }
        .btn-group { display: flex; flex-direction: column; gap: 12px; }
        .btn { display: inline-flex; align-items: center; justify-content: center; padding: 12px 20px; border-radius: 8px; font-size: 15px; font-weight: 600; text-decoration: none; cursor: pointer; transition: all 0.2s; }
        .btn-primary { background: #2563eb; color: white; }
        .btn-primary:hover { background: #1d4ed8; }
        .btn-secondary { background: #f3f4f6; color: #374151; border: 1px solid #d1d5db; }
        .btn-secondary:hover { background: #e5e7eb; }
    </style>
</head>
<body>
    <div class="card">
        <h2>🎉 初始化完成</h2>
        <p>管理员密码已成功设置！您可以立即进入管理后台配置存储、策略与其他高级选项，或前往前台开始使用。</p>
        <div class="btn-group">
            <a href="/#/admin/dashboard" class="btn btn-primary">进入管理后台设置 (/#/admin/dashboard)</a>
            <a href="/" class="btn btn-secondary">前往文件提取首页 (/)</a>
        </div>
    </div>
</body>
</html>"#;
        Ok(Html(success_html).into_response())
    }
}
