use async_trait::async_trait;

#[async_trait]
pub trait LlmClient: Send + Sync {
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
