use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use fcb_core::AppError;
use fcb_security::{verify_jwt_token, AdminClaims};
use crate::state::AppState;

#[derive(Clone, Debug)]
pub struct AdminAuth {
    pub token: String,
    pub claims: AdminClaims,
}

impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("缺少认证头".to_string()))?;

        let token = auth_header.strip_prefix("Bearer ").unwrap_or(auth_header).trim();
        let settings = state.config.get();

        let claims = verify_jwt_token(token, &settings.jwt_secret)?;
        Ok(AdminAuth {
            token: token.to_string(),
            claims,
        })
    }
}
