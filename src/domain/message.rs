use super::{ConversationId, MessageId, MessageRole, ToolCallId};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Message {
    pub id: MessageId,
    pub conversation_id: ConversationId,
    pub role: MessageRole,
    pub content: String,
    pub tool_call_id: Option<ToolCallId>,
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
            created_at: Utc::now(),
        }
    }

    pub fn new_tool_response(
        conversation_id: ConversationId,
        tool_call_id: ToolCallId,
        content: String,
    ) -> Self {
        Self {
            id: MessageId::new(),
            conversation_id,
            role: MessageRole::ToolResponse,
            content,
            tool_call_id: Some(tool_call_id),
            created_at: Utc::now(),
        }
    }
}
