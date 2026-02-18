use std::io;

use bytes::Bytes;
use futures::stream::BoxStream;

use crate::domain::StoragePath;

#[async_trait::async_trait]
pub trait StagingStore: Send + Sync {
    async fn store(
        &self,
        path: &StoragePath,
        stream: BoxStream<'_, Result<Bytes, io::Error>>,
        content_length: Option<u64>,
    ) -> Result<u64, StagingStoreError>;

    async fn fetch(&self, path: &StoragePath) -> Result<Vec<u8>, StagingStoreError>;

    async fn delete(&self, path: &StoragePath) -> Result<(), StagingStoreError>;

    async fn head(&self, path: &StoragePath) -> Result<u64, StagingStoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StagingStoreError {
    #[error("upload failed: {0}")]
    UploadFailed(String),
    #[error("object not found: {0}")]
    NotFound(String),
    #[error("download failed: {0}")]
    DownloadFailed(String),
    #[error("delete failed: {0}")]
    DeleteFailed(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}
