use crate::domain::{Message, MessageRole, ToolCall, ToolResult};

/// In-memory message representation used within the agent ReAct loop.
///
/// This is NOT the persisted domain `Message`; it carries richer tool metadata
/// needed by the LLM client's function-calling API. Conversation history is
/// hydrated from `Vec<Message>` via `From<Message>`.
#[derive(Debug, Clone)]
pub enum AgentMessage {
    System(String),
    User(String),
    Assistant {
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    ToolResult(ToolResult),
}

impl From<Message> for AgentMessage {
    fn from(msg: Message) -> Self {
        match msg.role {
            MessageRole::System => AgentMessage::System(msg.content),
            MessageRole::User => AgentMessage::User(msg.content),
            MessageRole::Assistant => AgentMessage::Assistant {
                content: Some(msg.content),
                tool_calls: Vec::new(),
            },
            MessageRole::Tool => AgentMessage::Assistant {
                content: None,
                tool_calls: Vec::new(),
            },
            MessageRole::ToolResponse => {
                // Reconstruct a minimal ToolResult from the persisted message.
                // tool_call_id and tool_name are best-effort from available fields.
                let tool_call_id = msg
                    .tool_call_id
                    .unwrap_or_else(|| crate::domain::ToolCallId::new("unknown"));
                AgentMessage::ToolResult(ToolResult {
                    tool_call_id,
                    tool_name: crate::domain::ToolName::new("unknown"),
                    content: msg.content,
                })
            }
        }
    }
}
