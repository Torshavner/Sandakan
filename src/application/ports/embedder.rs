use async_trait::async_trait;

use crate::domain::Embedding;

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Embedding, EmbedderError>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>, EmbedderError>;
}

#[derive(Debug, thiserror::Error)]
pub enum EmbedderError {
    #[error("embedding api request failed: {0}")]
    ApiRequestFailed(String),
    #[error("embedding rate limited")]
    RateLimited,
    #[error("invalid embedding response: {0}")]
    InvalidResponse(String),
    #[error("model loading failed: {0}")]
    ModelLoadFailed(String),
    #[error("inference failed: {0}")]
    InferenceFailed(String),
}
