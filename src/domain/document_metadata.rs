use super::{ContentType, Document};

/// Document-level context shared (Arc) across all chunks from the same source.
///
/// Used as a Flyweight — one instance per document, referenced by all its chunks.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentMetadata {
    pub title: String,
    pub content_type: ContentType,
    pub source_url: Option<String>,
}

impl DocumentMetadata {
    /// Derives metadata from a Document, stripping the file extension for the title.
    pub fn from_document(doc: &Document, source_url: Option<String>) -> Self {
        let title = strip_extension(&doc.filename);
        Self {
            title,
            content_type: doc.content_type,
            source_url,
        }
    }
}

fn strip_extension(filename: &str) -> String {
    match filename.rfind('.') {
        Some(pos) if pos > 0 => filename[..pos].to_string(),
        _ => filename.to_string(),
    }
}
