use crate::application::ports::{ConversationRepository, JobRepository, RepositoryError};
use crate::domain::{Conversation, ConversationId, Job, JobId, JobStatus, Message};

pub struct MockConversationRepository;

#[async_trait::async_trait]
impl ConversationRepository for MockConversationRepository {
    async fn create_conversation(
        &self,
        _conversation: &Conversation,
    ) -> Result<(), RepositoryError> {
        Ok(())
    }

    async fn get_conversation(
        &self,
        _id: ConversationId,
    ) -> Result<Option<Conversation>, RepositoryError> {
        Ok(None)
    }

    async fn append_message(&self, _message: &Message) -> Result<(), RepositoryError> {
        Ok(())
    }

    async fn get_messages(
        &self,
        _conversation_id: ConversationId,
        _limit: usize,
    ) -> Result<Vec<Message>, RepositoryError> {
        Ok(vec![])
    }
}

pub struct MockJobRepository;

#[async_trait::async_trait]
impl JobRepository for MockJobRepository {
    async fn create(&self, _job: &Job) -> Result<(), RepositoryError> {
        Ok(())
    }

    async fn get_by_id(&self, _id: JobId) -> Result<Option<Job>, RepositoryError> {
        Ok(None)
    }

    async fn update_status(
        &self,
        _id: JobId,
        _status: JobStatus,
        _error_message: Option<&str>,
    ) -> Result<(), RepositoryError> {
        Ok(())
    }

    async fn list_by_status(&self, _status: JobStatus) -> Result<Vec<Job>, RepositoryError> {
        Ok(vec![])
    }
}
