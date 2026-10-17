//! Application version constant.

/// Current FileHive application version.
pub const APP_VERSION: &str = "0.1.1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_value() {
        assert_eq!(APP_VERSION, "0.1.1");
    }
}
