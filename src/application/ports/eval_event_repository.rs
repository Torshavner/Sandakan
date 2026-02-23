use async_trait::async_trait;

use crate::domain::{EvalEvent, EvalEventId};

#[derive(Debug, thiserror::Error)]
pub enum EvalEventError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(String),
}

#[async_trait]
pub trait EvalEventRepository: Send + Sync {
    async fn record(&self, event: &EvalEvent) -> Result<(), EvalEventError>;
    async fn get(&self, id: EvalEventId) -> Result<Option<EvalEvent>, EvalEventError>;
    async fn list(&self, limit: Option<usize>) -> Result<Vec<EvalEvent>, EvalEventError>;
    async fn sample(&self, n: usize) -> Result<Vec<EvalEvent>, EvalEventError>;
}
