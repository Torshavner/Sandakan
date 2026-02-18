use crate::application::ports::{StagingStore, StagingStoreError};

pub struct MockStagingStore;

#[async_trait::async_trait]
impl StagingStore for MockStagingStore {
    async fn store(
        &self,
        _path: &crate::domain::StoragePath,
        _stream: futures::stream::BoxStream<'_, Result<bytes::Bytes, std::io::Error>>,
        _content_length: Option<u64>,
    ) -> Result<u64, StagingStoreError> {
        Ok(0)
    }

    async fn fetch(
        &self,
        _path: &crate::domain::StoragePath,
    ) -> Result<Vec<u8>, StagingStoreError> {
        Ok(vec![])
    }

    async fn delete(&self, _path: &crate::domain::StoragePath) -> Result<(), StagingStoreError> {
        Ok(())
    }

    async fn head(&self, _path: &crate::domain::StoragePath) -> Result<u64, StagingStoreError> {
        Ok(0)
    }
}
