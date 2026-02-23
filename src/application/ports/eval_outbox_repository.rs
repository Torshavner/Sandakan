use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{EvalEventId, EvalOutboxEntry};

#[derive(Debug, thiserror::Error)]
pub enum EvalOutboxError {
    #[error("database: {0}")]
    Database(String),
    #[error("serialization: {0}")]
    Serialization(String),
}

#[async_trait]
pub trait EvalOutboxRepository: Send + Sync {
    async fn enqueue(&self, eval_event_id: EvalEventId) -> Result<(), EvalOutboxError>;

    async fn claim_pending(
        &self,
        batch_size: usize,
    ) -> Result<Vec<EvalOutboxEntry>, EvalOutboxError>;

    async fn mark_done(&self, id: Uuid) -> Result<(), EvalOutboxError>;

    async fn mark_failed(&self, id: Uuid, error: &str) -> Result<(), EvalOutboxError>;
}
