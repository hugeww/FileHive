//! Utility functions for FileHive core.

use rand::Rng;

/// Characters used for secret alphanumeric pickup codes (uppercase letters and digits).
const SECRET_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// Sanitize an untrusted filename to prevent path traversal and injection attacks.
///
/// Steps:
/// 1. Strips null bytes (`\0`).
/// 2. Strips directory traversal sequences (`..`).
/// 3. Removes path separators (`/` and `\`).
/// 4. Strips any newly formed directory traversal sequences.
/// 5. Trims leading and trailing whitespace.
pub fn sanitize_filename(name: &str) -> String {
    // 1. Strip null bytes
    let mut s: String = name.chars().filter(|&c| c != '\0').collect();

    // 2. Strip directory traversal `..`
    while s.contains("..") {
        s = s.replace("..", "");
    }

    // 3. Remove path separators
    s = s.replace(['/', '\\'], "");

    // 4. Strip any newly formed `..` after separator removal
    while s.contains("..") {
        s = s.replace("..", "");
    }

    // 5. Trim whitespace
    s.trim().to_string()
}

/// Generates a random 5-digit number string between 10000 and 99999.
pub fn generate_number_code() -> String {
    let mut rng = rand::thread_rng();
    let num: u32 = rng.gen_range(10000..=99999);
    num.to_string()
}

/// Generates a random 5-character alphanumeric secret code (uppercase letters and digits).
pub fn generate_secret_code() -> String {
    let mut rng = rand::thread_rng();
    (0..5)
        .map(|_| {
            let idx = rng.gen_range(0..SECRET_ALPHABET.len());
            SECRET_ALPHABET[idx] as char
        })
        .collect()
}

/// Generates a pickup code based on `code_type`.
///
/// - `"number"` or `"num"`: Generates a random 5-digit number string (10000..=99999).
/// - `"secret"` or any other type: Generates a random 5-character secret string (uppercase letters and digits).
pub fn generate_code(code_type: &str) -> String {
    match code_type.trim().to_ascii_lowercase().as_str() {
        "number" | "num" => generate_number_code(),
        _ => generate_secret_code(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_normal() {
        assert_eq!(sanitize_filename("document.pdf"), "document.pdf");
        assert_eq!(sanitize_filename("my photo 2026.jpg"), "my photo 2026.jpg");
    }

    #[test]
    fn test_sanitize_filename_traversal() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "etcpasswd");
        assert_eq!(sanitize_filename("..\\..\\windows\\system32"), "windowssystem32");
        assert_eq!(sanitize_filename("....//....//test.txt"), "test.txt");
        assert_eq!(sanitize_filename(".."), "");
    }

    #[test]
    fn test_sanitize_filename_null_bytes() {
        assert_eq!(sanitize_filename("test\0file.txt"), "testfile.txt");
        assert_eq!(sanitize_filename("\0\0abc.tar.gz\0"), "abc.tar.gz");
    }

    #[test]
    fn test_sanitize_filename_spaces() {
        assert_eq!(sanitize_filename("   hello.txt   "), "hello.txt");
        assert_eq!(sanitize_filename("   "), "");
    }

    #[test]
    fn test_generate_code_number() {
        for _ in 0..100 {
            let code = generate_code("number");
            assert_eq!(code.len(), 5);
            let parsed: u32 = code.parse().expect("must be a valid integer");
            assert!((10000..=99999).contains(&parsed));
        }

        let code_num = generate_code("num");
        assert_eq!(code_num.len(), 5);
        assert!(code_num.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_generate_code_secret() {
        for _ in 0..100 {
            let code = generate_code("secret");
            assert_eq!(code.len(), 5);
            assert!(code.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
        }

        let code_default = generate_code("");
        assert_eq!(code_default.len(), 5);
        assert!(code_default.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    }
}
