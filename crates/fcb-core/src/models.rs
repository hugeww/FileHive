//! Data models and contracts for FileCodeBox core.

use serde::{Deserialize, Serialize};

/// Representation of a stored file in the storage backend.
///
/// Decouples storage operations from database and ORM models, matching the
/// Python FileCodeBox contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredFile {
    /// Relative or absolute base directory path for the file.
    pub file_path: String,

    /// Unique stored file name (typically UUID + extension).
    pub uuid_file_name: String,

    /// Associated pickup code.
    #[serde(default)]
    pub code: String,

    /// Key prefix (e.g. user directory or category).
    #[serde(default)]
    pub prefix: String,

    /// Key suffix / extension.
    #[serde(default)]
    pub suffix: String,

    /// Optional text content (when sharing text snippets rather than binary files).
    #[serde(default)]
    pub text: Option<String>,
}

impl StoredFile {
    /// Create a new `StoredFile` with the required path and uuid name, initializing
    /// optional metadata to empty defaults.
    pub fn new(file_path: impl Into<String>, uuid_file_name: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            uuid_file_name: uuid_file_name.into(),
            code: String::new(),
            prefix: String::new(),
            suffix: String::new(),
            text: None,
        }
    }

    /// Returns the combined storage path for the file.
    ///
    /// Formatted as `"{file_path}/{uuid_file_name}"`.
    pub fn get_file_path(&self) -> String {
        format!("{}/{}", self.file_path, self.uuid_file_name)
    }
}

impl Default for StoredFile {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            uuid_file_name: String::new(),
            code: String::new(),
            prefix: String::new(),
            suffix: String::new(),
            text: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stored_file_path() {
        let file = StoredFile::new("uploads/2026", "abc-123.png");
        assert_eq!(file.get_file_path(), "uploads/2026/abc-123.png");
    }

    #[test]
    fn test_stored_file_serialization() {
        let mut file = StoredFile::new("data", "test.txt");
        file.code = "12345".into();
        file.text = Some("hello world".into());

        let json = serde_json::to_string(&file).unwrap();
        let deserialized: StoredFile = serde_json::from_str(&json).unwrap();
        assert_eq!(file, deserialized);
        assert_eq!(deserialized.get_file_path(), "data/test.txt");
        assert_eq!(deserialized.text.as_deref(), Some("hello world"));
    }
}
