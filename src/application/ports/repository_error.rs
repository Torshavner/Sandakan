#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("query failed: {0}")]
    QueryFailed(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("constraint violation: {0}")]
    ConstraintViolation(String),
}
