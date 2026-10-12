//! Application version constant.

/// Current FileCodeBox application version.
pub const APP_VERSION: &str = "2.8.2";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_value() {
        assert_eq!(APP_VERSION, "2.8.2");
    }
}
