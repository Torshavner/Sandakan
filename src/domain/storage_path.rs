use std::fmt;

use super::chunk::DocumentId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePath(String);

impl StoragePath {
    pub fn new(document_id: &DocumentId, filename: &str) -> Self {
        Self(format!("{}/{}", document_id.as_uuid(), filename))
    }

    pub fn from_raw(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StoragePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
