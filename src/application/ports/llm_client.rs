use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

pub type LlmTokenStream = Pin<Box<dyn Stream<Item = Result<String, LlmClientError>> + Send>>;

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str, context: &str) -> Result<String, LlmClientError>;
    async fn complete_stream(
        &self,
        prompt: &str,
        context: &str,
    ) -> Result<LlmTokenStream, LlmClientError>;
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
