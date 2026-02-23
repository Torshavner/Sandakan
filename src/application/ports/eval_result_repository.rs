use async_trait::async_trait;

use crate::domain::EvalResult;

#[derive(Debug, thiserror::Error)]
pub enum EvalResultError {
    #[error("database: {0}")]
    Database(String),
    #[error("serialization: {0}")]
    Serialization(String),
}

#[async_trait]
pub trait EvalResultRepository: Send + Sync {
    async fn save(&self, result: &EvalResult) -> Result<(), EvalResultError>;
}
