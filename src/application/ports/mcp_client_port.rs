use async_trait::async_trait;

use crate::domain::{ToolCall, ToolResult};

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),
    #[error("tool execution failed: {0}")]
    ExecutionFailed(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("MCP server process exited unexpectedly")]
    ServerExited,
}

#[async_trait]
pub trait McpClientPort: Send + Sync {
    async fn call_tool(&self, call: &ToolCall) -> Result<ToolResult, McpError>;
}
