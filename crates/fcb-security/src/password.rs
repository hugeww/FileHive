use rand::Rng;
use scrypt::{scrypt, Params};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const SCRYPT_LOG_N: u8 = 14; // 2^14 = 16384
pub const SCRYPT_N: u64 = 16384;
pub const SCRYPT_R: u32 = 8;
pub const SCRYPT_P: u32 = 1;
pub const SCRYPT_LEN: usize = 64;

pub const LEGACY_DEFAULT_ADMIN_TOKEN: &str = "FileCodeBox2023";

/// 使用与 Python 一致的 scrypt 参数对密码哈希
/// 格式: scrypt$<n>$<r>$<p>$<salt_hex>$<hash_hex>
pub fn hash_password(password: &str) -> String {
    let mut salt_bytes = [0u8; 16];
    rand::thread_rng().fill(&mut salt_bytes);
    let salt_hex = hex::encode(salt_bytes);

    let params = Params::new(SCRYPT_LOG_N, SCRYPT_R, SCRYPT_P, SCRYPT_LEN)
        .expect("Valid scrypt params");

    let mut output = [0u8; SCRYPT_LEN];
    scrypt(password.as_bytes(), salt_hex.as_bytes(), &params, &mut output)
        .expect("scrypt hash failed");

    let hash_hex = hex::encode(output);
    format!("scrypt${}${}${}${}${}", SCRYPT_N, SCRYPT_R, SCRYPT_P, salt_hex, hash_hex)
}

/// 校验密码：支持 scrypt、sha256 以及旧明文（兼容 Python 版）
pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    if stored_hash.is_empty() {
        return false;
    }

    if stored_hash.starts_with("scrypt$") {
        let parts: Vec<&str> = stored_hash.split('$').collect();
        if parts.len() != 6 {
            return false;
        }
        let n: u64 = match parts[1].parse() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let r: u32 = match parts[2].parse() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let p: u32 = match parts[3].parse() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let salt = parts[4];
        let expected_hash = parts[5];

        // 必须为 2 的幂且 > 1
        if n <= 1 || (n & (n - 1)) != 0 {
            return false;
        }
        let log_n = n.trailing_zeros() as u8;

        let params = match Params::new(log_n, r, p, SCRYPT_LEN) {
            Ok(params) => params,
            Err(_) => return false,
        };

        let mut output = [0u8; SCRYPT_LEN];
        if scrypt(password.as_bytes(), salt.as_bytes(), &params, &mut output).is_err() {
            return false;
        }
        let computed_hex = hex::encode(output);
        return computed_hex.as_bytes().ct_eq(expected_hash.as_bytes()).into();
    }

    if stored_hash.starts_with("sha256$") {
        let parts: Vec<&str> = stored_hash.split('$').collect();
        if parts.len() != 3 {
            return false;
        }
        let salt = parts[1];
        let expected_hash = parts[2];

        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        hasher.update(password.as_bytes());
        let computed = hex::encode(hasher.finalize());
        return computed.as_bytes().ct_eq(expected_hash.as_bytes()).into();
    }

    // 历史明文兼容
    password.as_bytes().ct_eq(stored_hash.as_bytes()).into()
}

pub fn is_password_hashed(stored: &str) -> bool {
    (stored.starts_with("scrypt$") && stored.split('$').count() == 6)
        || (stored.starts_with("sha256$") && stored.split('$').count() == 3)
}

/// 检查密码是否需要升级为 scrypt（当存储为 sha256 或历史明文时返回 true）
pub fn password_needs_rehash(stored_hash: &str) -> bool {
    !stored_hash.is_empty() && !stored_hash.starts_with("scrypt$")
}

/// 检查 token 是否为历史默认口令（"FileCodeBox2023"）
pub fn is_legacy_default_token(token: &str) -> bool {
    verify_password(LEGACY_DEFAULT_ADMIN_TOKEN, token)
}

/// 检查管理员 token 是否已初始化（廉价探测，避免对 scrypt 跑慢哈希）
pub fn is_config_initialized(admin_token: &str) -> bool {
    let token = admin_token.trim();
    if token.is_empty() {
        return false;
    }
    if is_password_hashed(token) && token.starts_with("scrypt$") {
        return true;
    }
    !is_legacy_default_token(token)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_roundtrip() {
        let pass = "Integration-Test-12345";
        let h = hash_password(pass);
        assert!(is_password_hashed(&h));
        assert!(verify_password(pass, &h));
        assert!(!verify_password("wrong-password", &h));
    }

    #[test]
    fn test_legacy_default_token() {
        assert_eq!(LEGACY_DEFAULT_ADMIN_TOKEN, "FileCodeBox2023");
        assert!(verify_password("FileCodeBox2023", LEGACY_DEFAULT_ADMIN_TOKEN));
        assert!(!verify_password("wrong", LEGACY_DEFAULT_ADMIN_TOKEN));
    }

    #[test]
    fn test_python_compatibility() {
        let python_vectors = [
            (
                "FileCodeBox2023",
                "scrypt$16384$8$1$6c8411553abbe529d77db4b5e4bd7d04$8775e6d9a55d6ac91085d879978ca6356b79872b36281591101515e92cc923636ed2308fab1cdae95f3fa97265382ccee8fc9920b2734e6511934e4edce503b5",
            ),
            (
                "admin123",
                "scrypt$16384$8$1$17f2c5ef6f9f680e002b485a0d4ad89c$2b79b4f31c69c17c822b896a8bc0eea2ad8c2f6acf15f49435b61d1271f456dca49bf5e8dcf489bf4c764034eae7402cefe030beb5d4bfb272c888b4db0f9fa7",
            ),
            (
                "Special!@#$%^&*()_+-=[]{}|;:,.<>?",
                "scrypt$16384$8$1$cab2551703d4bcd2ce01a7ecb7157603$e783d2b9513444d66a890ff446585ed032547a47f29d01b27f18cfb9ab513ded80801beb577b862573332e337c3be134ed5ce9b219bdce0b7715ae7c5a6d0312",
            ),
            (
                "中文密码测试123",
                "scrypt$16384$8$1$1a5e581fc7f73e16374bc91dff11837d$8216e20aa0bd1a7df61f5f5c67bfd6f58b153b96f8d7f039d8b0066276c71888654f95a86f7535a11a55fe5873550ee9d0bd2de86326994f36f546f2948af871",
            ),
            (
                "",
                "scrypt$16384$8$1$58e4243541f908aada427873e5a24802$7dd6e51ab7651da1e3b6c549cf53b4b9dc103d276eceb5370ed26d6d9208cd612e6c1444e2825e1dbea44ebc9fb78cfd05e6ee030408c4a1f76b3cf24b5724e7",
            ),
        ];

        for (pass, stored) in python_vectors {
            assert!(is_password_hashed(stored), "Should be identified as hashed: {}", stored);
            assert!(verify_password(pass, stored), "Failed to verify Python hash for: {}", pass);
            assert!(!verify_password(&format!("{}-wrong", pass), stored), "Should fail for wrong pass");
        }
    }


    #[test]
    fn test_legacy_sha256_and_plaintext() {
        let salt = "abcd1234";
        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        hasher.update(b"legacy_pass");
        let sha_hash = format!("sha256${}${}", salt, hex::encode(hasher.finalize()));

        assert!(is_password_hashed(&sha_hash));
        assert!(verify_password("legacy_pass", &sha_hash));
        assert!(!verify_password("other", &sha_hash));

        assert!(!is_password_hashed("plain_pass"));
        assert!(verify_password("plain_pass", "plain_pass"));
        assert!(!verify_password("plain_pass", "different"));
    }

    #[test]
    fn test_malformed_hashes() {
        assert!(!verify_password("x", "scrypt$1$2$3"));
        assert!(!verify_password("x", "scrypt$n$r$p$salt$zzz"));
        assert!(!verify_password("x", "scrypt$99999999999999999999999999$8$1$aa$bb"));
        assert!(!verify_password("x", "sha256$only-two"));
        assert!(!verify_password("x", ""));
    }

    #[test]
    fn test_config_initialized_and_rehash() {
        let scrypt_hash = hash_password("real-pass");
        let sha_hash = "sha256$salt$hash";

        assert!(is_config_initialized(&scrypt_hash));
        assert!(!password_needs_rehash(&scrypt_hash));

        assert!(is_config_initialized("my-custom-plaintext"));
        assert!(password_needs_rehash("my-custom-plaintext"));

        assert!(!is_config_initialized(""));
        assert!(!is_config_initialized(LEGACY_DEFAULT_ADMIN_TOKEN));
        assert!(is_legacy_default_token(LEGACY_DEFAULT_ADMIN_TOKEN));

        assert!(password_needs_rehash(sha_hash));
        assert!(!password_needs_rehash(""));
    }

}

