use super::{ConversationId, MessageId, MessageRole, ToolCallId, ToolName};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Message {
    pub id: MessageId,
    pub conversation_id: ConversationId,
    pub role: MessageRole,
    pub content: String,
    pub tool_call_id: Option<ToolCallId>,
    pub tool_name: Option<ToolName>,
    pub created_at: DateTime<Utc>,
}

impl Message {
    pub fn new(conversation_id: ConversationId, role: MessageRole, content: String) -> Self {
        Self {
            id: MessageId::new(),
            conversation_id,
            role,
            content,
            tool_call_id: None,
            tool_name: None,
            created_at: Utc::now(),
        }
    }

    /// Constructs a message representing the assistant's tool-call intent.
    /// `content` is the JSON-serialised `Vec<ToolCall>` for replay.
    pub fn new_tool_call(
        conversation_id: ConversationId,
        tool_name: ToolName,
        content: String,
    ) -> Self {
        Self {
            id: MessageId::new(),
            conversation_id,
            role: MessageRole::Tool,
            content,
            tool_call_id: None,
            tool_name: Some(tool_name),
            created_at: Utc::now(),
        }
    }

    pub fn new_tool_response(
        conversation_id: ConversationId,
        tool_call_id: ToolCallId,
        tool_name: ToolName,
        content: String,
    ) -> Self {
        Self {
            id: MessageId::new(),
            conversation_id,
            role: MessageRole::ToolResponse,
            content,
            tool_call_id: Some(tool_call_id),
            tool_name: Some(tool_name),
            created_at: Utc::now(),
        }
    }
}
