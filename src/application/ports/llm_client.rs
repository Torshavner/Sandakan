use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::domain::ToolCall;

use super::agent_message::AgentMessage;

pub type LlmTokenStream = Pin<Box<dyn Stream<Item = Result<String, LlmClientError>> + Send>>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug)]
pub enum LlmToolResponse {
    ToolCalls(Vec<ToolCall>),
    Content(String),
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str, context: &str) -> Result<String, LlmClientError>;

    async fn complete_stream(
        &self,
        prompt: &str,
        context: &str,
    ) -> Result<LlmTokenStream, LlmClientError>;

    async fn complete_with_tools(
        &self,
        messages: &[AgentMessage],
        tools: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError>;
}

#[derive(Debug, thiserror::Error)]
pub enum LlmClientError {
    #[error("api request failed: {0}")]
    ApiRequestFailed(String),
    #[error("rate limited")]
    RateLimited,
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("tool call parsing failed: {0}")]
    ToolCallParsing(String),
}
