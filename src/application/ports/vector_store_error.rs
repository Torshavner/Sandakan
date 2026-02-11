#[derive(Debug, thiserror::Error)]
pub enum VectorStoreError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("collection creation failed: {0}")]
    CollectionCreationFailed(String),
    #[error("collection deletion failed: {0}")]
    CollectionDeletionFailed(String),
    #[error("payload index creation failed: {0}")]
    PayloadIndexFailed(String),
    #[error("upsert failed: {0}")]
    UpsertFailed(String),
    #[error("search failed: {0}")]
    SearchFailed(String),
    #[error("delete failed: {0}")]
    DeleteFailed(String),
}
