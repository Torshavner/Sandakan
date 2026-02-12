use super::chunk::DocumentId;

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub id: DocumentId,
    pub filename: String,
    pub content_type: ContentType,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentType {
    Pdf,
    Audio,
    Text,
}

impl ContentType {
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            "application/pdf" => Some(Self::Pdf),
            m if m.starts_with("audio/") => Some(Self::Audio),
            "text/plain" => Some(Self::Text),
            _ => None,
        }
    }

    pub fn as_mime(&self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Audio => "audio/mpeg",
            Self::Text => "text/plain",
        }
    }
}

impl Document {
    pub fn new(filename: String, content_type: ContentType, size_bytes: u64) -> Self {
        Self {
            id: DocumentId::new(),
            filename,
            content_type,
            size_bytes,
        }
    }
}
