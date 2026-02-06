use async_trait::async_trait;

use crate::domain::Embedding;

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Embedding, LlmClientError>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>, LlmClientError>;
    async fn complete(&self, prompt: &str, context: &str) -> Result<String, LlmClientError>;
}

#[derive(Debug, thiserror::Error)]
pub enum LlmClientError {
    #[error("api request failed: {0}")]
    ApiRequestFailed(String),
    #[error("rate limited")]
    RateLimited,
    #[error("invalid response: {0}")]
    InvalidResponse(String),
}
