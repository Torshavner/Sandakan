use crate::application::ports::{LlmClientError, McpError, RepositoryError};

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("max iterations ({0}) exceeded without final answer")]
    MaxIterationsExceeded(usize),
    #[error("llm error: {0}")]
    Llm(#[from] LlmClientError),
    #[error("tool execution error: {0}")]
    Tool(String),
    #[error("repository error: {0}")]
    Repository(String),
}

impl From<RepositoryError> for AgentError {
    fn from(e: RepositoryError) -> Self {
        AgentError::Repository(e.to_string())
    }
}

impl From<McpError> for AgentError {
    fn from(e: McpError) -> Self {
        AgentError::Tool(e.to_string())
    }
}
