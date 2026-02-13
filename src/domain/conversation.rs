use super::{ConversationId, Message};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Conversation {
    pub id: ConversationId,
    pub title: Option<String>,
    pub messages: Vec<Message>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Conversation {
    pub fn new(title: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: ConversationId::new(),
            title,
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}
