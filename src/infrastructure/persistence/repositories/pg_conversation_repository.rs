use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use tracing::instrument;

use crate::application::ports::{ConversationRepository, RepositoryError};
use crate::domain::{Conversation, ConversationId, Message, MessageId, MessageRole};

pub struct PgConversationRepository {
    pool: PgPool,
}

impl PgConversationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConversationRepository for PgConversationRepository {
    #[instrument(skip(self, conversation), fields(conversation_id = %conversation.id.as_uuid()))]
    async fn create_conversation(
        &self,
        conversation: &Conversation,
    ) -> Result<(), RepositoryError> {
        let conversation_id = conversation.id.as_uuid();

        sqlx::query!(
            r#"
            INSERT INTO conversations (id, title, created_at, updated_at)
            VALUES ($1, $2, $3, $4)
            "#,
            conversation_id,
            conversation.title,
            conversation.created_at,
            conversation.updated_at
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::QueryFailed(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self), fields(conversation_id = %id.as_uuid()))]
    async fn get_conversation(
        &self,
        id: ConversationId,
    ) -> Result<Option<Conversation>, RepositoryError> {
        let conversation_id = id.as_uuid();

        let row = sqlx::query!(
            r#"
            SELECT id, title, created_at, updated_at
            FROM conversations
            WHERE id = $1
            "#,
            conversation_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::QueryFailed(e.to_string()))?;

        match row {
            Some(r) => {
                let messages = self.get_messages(id, 1000).await?;

                Ok(Some(Conversation {
                    id: ConversationId::from_uuid(r.id),
                    title: r.title,
                    messages,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    #[instrument(skip(self, message), fields(message_id = %message.id.as_uuid(), conversation_id = %message.conversation_id.as_uuid()))]
    async fn append_message(&self, message: &Message) -> Result<(), RepositoryError> {
        let message_id = message.id.as_uuid();
        let conversation_id = message.conversation_id.as_uuid();
        let role = message.role.as_str();

        sqlx::query!(
            r#"
            INSERT INTO messages (id, conversation_id, role, content, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            message_id,
            conversation_id,
            role,
            message.content,
            message.created_at
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::QueryFailed(e.to_string()))?;

        let now = Utc::now();
        sqlx::query!(
            r#"
            UPDATE conversations
            SET updated_at = $1
            WHERE id = $2
            "#,
            now,
            conversation_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::QueryFailed(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self), fields(conversation_id = %conversation_id.as_uuid(), limit = %limit))]
    async fn get_messages(
        &self,
        conversation_id: ConversationId,
        limit: usize,
    ) -> Result<Vec<Message>, RepositoryError> {
        let conv_id = conversation_id.as_uuid();
        let limit_i64 = limit as i64;

        let rows = sqlx::query!(
            r#"
            SELECT id, conversation_id, role, content, created_at
            FROM messages
            WHERE conversation_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            conv_id,
            limit_i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::QueryFailed(e.to_string()))?;

        let mut messages: Vec<Message> = rows
            .into_iter()
            .map(|r| {
                let role = r
                    .role
                    .parse::<MessageRole>()
                    .map_err(RepositoryError::QueryFailed)?;

                Ok(Message {
                    id: MessageId::from_uuid(r.id),
                    conversation_id: ConversationId::from_uuid(r.conversation_id),
                    role,
                    content: r.content,
                    created_at: r.created_at,
                })
            })
            .collect::<Result<Vec<_>, RepositoryError>>()?;

        messages.reverse();
        Ok(messages)
    }
}
