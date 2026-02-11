use async_trait::async_trait;

use crate::domain::Chunk;

#[async_trait]
pub trait TextSplitter: Send + Sync {
    async fn split(
        &self,
        text: &str,
        document_id: crate::domain::DocumentId,
    ) -> Result<Vec<Chunk>, TextSplitterError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TextSplitterError {
    #[error("tokenization failed: {0}")]
    TokenizationFailed(String),
    #[error("splitting failed: {0}")]
    SplittingFailed(String),
}
