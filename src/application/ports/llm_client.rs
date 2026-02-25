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

    /// Stream tokens from a full conversation history.
    ///
    /// Use when the caller needs a real LLM token stream from structured
    /// message history (e.g. direct streaming endpoints). `AgentService`
    /// does NOT call this — it fake-streams the already-buffered ReAct answer
    /// to avoid a redundant LLM round-trip.
    async fn complete_stream_with_messages(
        &self,
        messages: &[AgentMessage],
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
