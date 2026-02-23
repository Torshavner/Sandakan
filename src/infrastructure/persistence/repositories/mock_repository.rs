use uuid::Uuid;

use crate::application::ports::{
    ConversationRepository, EvalEventError, EvalEventRepository, EvalOutboxError,
    EvalOutboxRepository, EvalResultError, EvalResultRepository, JobRepository, RepositoryError,
};
use crate::domain::{
    Conversation, ConversationId, EvalEvent, EvalEventId, EvalOutboxEntry, EvalResult, Job, JobId,
    JobStatus, Message,
};

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

pub struct MockEvalEventRepository;

#[async_trait::async_trait]
impl EvalEventRepository for MockEvalEventRepository {
    async fn record(&self, _event: &EvalEvent) -> Result<(), EvalEventError> {
        Ok(())
    }

    async fn get(&self, _id: EvalEventId) -> Result<Option<EvalEvent>, EvalEventError> {
        Ok(None)
    }

    async fn list(&self, _limit: Option<usize>) -> Result<Vec<EvalEvent>, EvalEventError> {
        Ok(vec![])
    }

    async fn sample(&self, _n: usize) -> Result<Vec<EvalEvent>, EvalEventError> {
        Ok(vec![])
    }
}

pub struct MockEvalOutboxRepository;

#[async_trait::async_trait]
impl EvalOutboxRepository for MockEvalOutboxRepository {
    async fn enqueue(&self, _eval_event_id: EvalEventId) -> Result<(), EvalOutboxError> {
        Ok(())
    }

    async fn claim_pending(
        &self,
        _batch_size: usize,
    ) -> Result<Vec<EvalOutboxEntry>, EvalOutboxError> {
        Ok(vec![])
    }

    async fn mark_done(&self, _id: Uuid) -> Result<(), EvalOutboxError> {
        Ok(())
    }

    async fn mark_failed(&self, _id: Uuid, _error: &str) -> Result<(), EvalOutboxError> {
        Ok(())
    }
}

pub struct MockEvalResultRepository;

#[async_trait::async_trait]
impl EvalResultRepository for MockEvalResultRepository {
    async fn save(&self, _result: &EvalResult) -> Result<(), EvalResultError> {
        Ok(())
    }
}
