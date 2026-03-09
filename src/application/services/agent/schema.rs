use crate::application::errors::AgentError;
use crate::application::ports::LlmTokenStream;
use crate::domain::ConversationId;

pub struct AgentChatRequest {
    pub conversation_id: Option<ConversationId>,
    pub user_message: String,
    pub correlation_id: Option<String>,
}

pub struct AgentChatResponse {
    pub progress_rx: tokio::sync::mpsc::Receiver<AgentProgressEvent>,
    /// Real token-by-token stream of the final LLM answer.
    pub token_stream: LlmTokenStream,
    pub conversation_id: ConversationId,
}

/// Events emitted during the ReAct loop that the presentation layer forwards
/// as SSE progress messages before the final token stream begins.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentProgressEvent {
    Thinking {
        iteration: usize,
    },
    ToolCall {
        name: String,
    },
    ToolResult {
        name: String,
        truncated_content: String,
    },
    Reflection {
        score: f32,
        needs_correction: bool,
        issues: Vec<String>,
    },
    CorrectionApplied,
}

// ─── Port (thin trait for AppState to avoid 5th generic) ─────────────────────

#[async_trait::async_trait]
pub trait AgentServicePort: Send + Sync {
    async fn chat(&self, request: AgentChatRequest) -> Result<AgentChatResponse, AgentError>;
}
