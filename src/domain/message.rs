use super::{ConversationId, MessageId, MessageRole};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Message {
    pub id: MessageId,
    pub conversation_id: ConversationId,
    pub role: MessageRole,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

impl Message {
    pub fn new(conversation_id: ConversationId, role: MessageRole, content: String) -> Self {
        Self {
            id: MessageId::new(),
            conversation_id,
            role,
            content,
            created_at: Utc::now(),
        }
    }
}
