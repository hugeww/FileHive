use fcb_core::AppError;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};

pub const JWT_SECRET_MIN_LENGTH: usize = 32;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

// 别名保留以防兼容需要
pub type AdminClaims = Claims;

/// 生成 48 字节 URL-safe 随机字符串（与 Python secrets.token_urlsafe(48) 一致）
pub fn generate_jwt_secret() -> String {
    const URL_SAFE_CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut rng = rand::thread_rng();
    (0..48)
        .map(|_| {
            let idx = rng.gen_range(0..URL_SAFE_CHARSET.len());
            URL_SAFE_CHARSET[idx] as char
        })
        .collect()
}


pub fn is_valid_jwt_secret(secret: &str) -> bool {
    secret.len() >= JWT_SECRET_MIN_LENGTH
}

/// 创建 JWT token
pub fn create_token(secret: &str, expire_secs: i64) -> Result<String, AppError> {
    if !is_valid_jwt_secret(secret) {
        return Err(AppError::Internal(
            "JWT签名密钥未初始化或长度小于32位".to_string(),
        ));
    }

    let now = chrono::Utc::now().timestamp();
    let exp = (now + expire_secs).max(0) as usize;

    let claims = Claims {
        sub: "admin".to_string(),
        exp,
        iat: now as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Failed to sign JWT: {}", e)))
}


/// 兼容别名
pub fn create_jwt_token(secret: &str, expire_secs: i64) -> Result<String, AppError> {
    create_token(secret, expire_secs)
}

/// 验证 JWT token
pub fn verify_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let token_clean = token.strip_prefix("Bearer ").unwrap_or(token).trim();

    let mut validation = Validation::default();
    validation.leeway = 0;

    decode::<Claims>(
        token_clean,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Unauthorized(format!("无效或过期的管理员凭证: {}", e)))
}


/// 兼容别名
pub fn verify_jwt_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    verify_token(token, secret)
}

pub fn get_select_token(code: &str, secret: &str, offset: i64) -> String {
    use sha2::{Digest, Sha256};
    let now_secs = chrono::Utc::now().timestamp();
    let time_factor = (now_secs / 1000) - offset.max(0);
    let mut hasher = Sha256::new();
    hasher.update(format!("{}{}{}{}", code, time_factor, "000", secret).as_bytes());
    hex::encode(hasher.finalize())
}

pub fn verify_select_token(code: &str, token: &str, secret: &str) -> bool {
    use subtle::ConstantTimeEq;
    let t0 = get_select_token(code, secret, 0);
    let t1 = get_select_token(code, secret, 1);
    let token_bytes = token.as_bytes();
    let t0_bytes = t0.as_bytes();
    let t1_bytes = t1.as_bytes();

    let eq0: bool = if token_bytes.len() == t0_bytes.len() {
        token_bytes.ct_eq(t0_bytes).into()
    } else {
        false
    };
    let eq1: bool = if token_bytes.len() == t1_bytes.len() {
        token_bytes.ct_eq(t1_bytes).into()
    } else {
        false
    };
    eq0 || eq1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_secret_generation() {
        let secret = generate_jwt_secret();
        assert_eq!(secret.len(), 48);
        assert!(is_valid_jwt_secret(&secret));
        assert!(!is_valid_jwt_secret("short"));
    }

    #[test]
    fn test_jwt_roundtrip() {
        let secret = generate_jwt_secret();
        let token = create_token(&secret, 3600).unwrap();
        let claims = verify_token(&token, &secret).unwrap();
        assert_eq!(claims.sub, "admin");
        assert!(claims.exp > claims.iat);

        // Bearer prefix support
        let bearer_token = format!("Bearer {}", token);
        let claims2 = verify_token(&bearer_token, &secret).unwrap();
        assert_eq!(claims2.sub, "admin");

        // Invalid secret
        let wrong_secret = generate_jwt_secret();
        assert!(verify_token(&token, &wrong_secret).is_err());
    }

    #[test]
    fn test_expired_token() {
        let secret = generate_jwt_secret();
        // expire immediately
        let token = create_token(&secret, -10).unwrap();
        assert!(verify_token(&token, &secret).is_err());
    }

    #[test]
    fn test_select_token() {
        let secret = "my-secret-key-must-be-long-enough-32-chars";
        let code = "12345";
        let token = get_select_token(code, secret, 0);
        assert!(!token.is_empty());
        assert!(verify_select_token(code, &token, secret));
        assert!(!verify_select_token("54321", &token, secret));
        assert!(!verify_select_token(code, "invalid-token", secret));
    }
}


