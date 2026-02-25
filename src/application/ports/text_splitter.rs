use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::{Chunk, DocumentId, DocumentMetadata, TranscriptSegment};

#[async_trait]
pub trait TextSplitter: Send + Sync {
    async fn split(
        &self,
        text: &str,
        document_id: DocumentId,
        metadata: Option<Arc<DocumentMetadata>>,
    ) -> Result<Vec<Chunk>, TextSplitterError>;

    /// Splits a sequence of timed transcript segments into token-budgeted chunks.
    ///
    /// Each output chunk inherits the `start_time` of the first segment that contributed to it,
    /// enabling deep-link timestamp citations in the retrieval response.
    async fn split_segments(
        &self,
        segments: &[TranscriptSegment],
        document_id: DocumentId,
        metadata: Option<Arc<DocumentMetadata>>,
    ) -> Result<Vec<Chunk>, TextSplitterError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TextSplitterError {
    #[error("tokenization failed: {0}")]
    TokenizationFailed(String),
    #[error("splitting failed: {0}")]
    SplittingFailed(String),
}
