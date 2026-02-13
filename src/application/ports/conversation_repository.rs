use crate::domain::{Conversation, ConversationId, Message};
use async_trait::async_trait;

use super::RepositoryError;

#[async_trait]
pub trait ConversationRepository: Send + Sync {
    async fn create_conversation(&self, conversation: &Conversation)
    -> Result<(), RepositoryError>;

    async fn get_conversation(
        &self,
        id: ConversationId,
    ) -> Result<Option<Conversation>, RepositoryError>;

    async fn append_message(&self, message: &Message) -> Result<(), RepositoryError>;

    async fn get_messages(
        &self,
        conversation_id: ConversationId,
        limit: usize,
    ) -> Result<Vec<Message>, RepositoryError>;
}
