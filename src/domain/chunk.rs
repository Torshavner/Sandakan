use std::sync::Arc;

use uuid::Uuid;

use super::document_metadata::DocumentMetadata;

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub id: ChunkId,
    pub text: String,
    pub document_id: DocumentId,
    pub page: Option<u32>,
    pub offset: usize,
    pub metadata: Option<Arc<DocumentMetadata>>,
    /// Start time in seconds of the first transcript segment that contributed to this chunk.
    /// `None` for non-media sources (PDF, plain text).
    pub start_time: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkId(Uuid);

impl ChunkId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for ChunkId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(Uuid);

impl DocumentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for DocumentId {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunk {
    /// Creates a chunk without document metadata (backward-compatible).
    pub fn new(text: String, document_id: DocumentId, page: Option<u32>, offset: usize) -> Self {
        Self {
            id: ChunkId::new(),
            text,
            document_id,
            page,
            offset,
            metadata: None,
            start_time: None,
        }
    }

    /// Creates a chunk with document-level metadata attached.
    pub fn with_metadata(
        text: String,
        document_id: DocumentId,
        page: Option<u32>,
        offset: usize,
        metadata: Arc<DocumentMetadata>,
    ) -> Self {
        Self {
            id: ChunkId::new(),
            text,
            document_id,
            page,
            offset,
            metadata: Some(metadata),
            start_time: None,
        }
    }

    /// Builder-style method to attach a media timestamp (seconds from the start of the file).
    pub fn with_start_time(mut self, start_time: f32) -> Self {
        self.start_time = Some(start_time);
        self
    }

    /// Returns an embedding-ready string that includes document context when available.
    ///
    /// Enriching embeddings with title and page/time guides the model toward semantically
    /// distinct vectors for identical text from different documents.
    pub fn as_contextual_string(&self) -> String {
        match &self.metadata {
            Some(meta) => {
                let location_label = match (self.page, self.start_time) {
                    (_, Some(t)) => format!("{:.1}s", t),
                    (Some(p), None) => p.to_string(),
                    (None, None) => "N/A".to_string(),
                };
                format!(
                    "Title: {}\nPage: {}\nContent: {}",
                    meta.title, location_label, self.text
                )
            }
            None => self.text.clone(),
        }
    }
}
