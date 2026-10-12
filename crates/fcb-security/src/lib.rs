//! fcb-security: FileCodeBox 密码哈希、JWT 认证与 SSRF 安全防护 crate
//!
//! 提供与 Python 原版 100% 兼容的安全性功能：
//! - Password hashing: scrypt 格式 (`scrypt$16384$8$1${salt_hex}${hash_hex}`),
//!   并支持历史 sha256 格式与明文兼容校验。
//! - JWT: HS256 签名与验证，支持 Claims (`sub`, `exp`, `iat`)。
//! - SSRF 防护: 阻断内网/回环/保留 IP/特定主机后缀/IP 简写绕过。

pub mod jwt;
pub mod password;
pub mod ssrf;

pub use jwt::{
    create_jwt_token, create_token, generate_jwt_secret, get_select_token, is_valid_jwt_secret,
    verify_jwt_token, verify_select_token, verify_token, AdminClaims, Claims,
    JWT_SECRET_MIN_LENGTH,
};
pub use password::{
    hash_password, is_config_initialized, is_legacy_default_token, is_password_hashed,
    password_needs_rehash, verify_password, LEGACY_DEFAULT_ADMIN_TOKEN, SCRYPT_LEN, SCRYPT_LOG_N,
    SCRYPT_N, SCRYPT_P, SCRYPT_R,
};
pub use ssrf::{
    is_endpoint_host_denied, is_ip_denied, validate_outbound_endpoint, validate_outbound_hostname,
    ENDPOINT_HOST_DENY_SUFFIXES,
};
