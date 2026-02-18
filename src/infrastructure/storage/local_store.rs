use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use futures::StreamExt;
use futures::stream::BoxStream;
use object_store::local::LocalFileSystem;
use object_store::path::Path as StorePath;
use object_store::{MultipartUpload, ObjectStore, PutPayload};

use crate::application::ports::{StagingStore, StagingStoreError};
use crate::domain::StoragePath;

pub struct LocalStagingStore {
    inner: Arc<LocalFileSystem>,
}

impl LocalStagingStore {
    pub fn new(base_path: PathBuf) -> Result<Self, StagingStoreError> {
        std::fs::create_dir_all(&base_path).map_err(StagingStoreError::Io)?;
        let fs = LocalFileSystem::new_with_prefix(base_path)
            .map_err(|e| StagingStoreError::UploadFailed(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(fs),
        })
    }
}

#[async_trait::async_trait]
impl StagingStore for LocalStagingStore {
    async fn store(
        &self,
        path: &StoragePath,
        mut stream: BoxStream<'_, Result<Bytes, io::Error>>,
        _content_length: Option<u64>,
    ) -> Result<u64, StagingStoreError> {
        let store_path = StorePath::from(path.as_str());
        let mut upload = self
            .inner
            .put_multipart(&store_path)
            .await
            .map_err(|e| StagingStoreError::UploadFailed(e.to_string()))?;

        let mut total_bytes: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let bytes = match chunk {
                Ok(b) => b,
                Err(e) => {
                    let _ = upload.abort().await;
                    return Err(StagingStoreError::Io(e));
                }
            };
            total_bytes += bytes.len() as u64;
            if let Err(e) = upload.put_part(PutPayload::from(bytes)).await {
                let _ = upload.abort().await;
                return Err(StagingStoreError::UploadFailed(e.to_string()));
            }
        }

        upload
            .complete()
            .await
            .map_err(|e| StagingStoreError::UploadFailed(e.to_string()))?;

        Ok(total_bytes)
    }

    async fn fetch(&self, path: &StoragePath) -> Result<Vec<u8>, StagingStoreError> {
        let store_path = StorePath::from(path.as_str());
        let result = self
            .inner
            .get(&store_path)
            .await
            .map_err(|e| StagingStoreError::NotFound(e.to_string()))?;

        let bytes = result
            .bytes()
            .await
            .map_err(|e| StagingStoreError::DownloadFailed(e.to_string()))?;

        Ok(bytes.to_vec())
    }

    async fn delete(&self, path: &StoragePath) -> Result<(), StagingStoreError> {
        let store_path = StorePath::from(path.as_str());
        self.inner
            .delete(&store_path)
            .await
            .map_err(|e| StagingStoreError::DeleteFailed(e.to_string()))
    }

    async fn head(&self, path: &StoragePath) -> Result<u64, StagingStoreError> {
        let store_path = StorePath::from(path.as_str());
        let meta = self
            .inner
            .head(&store_path)
            .await
            .map_err(|e| StagingStoreError::NotFound(e.to_string()))?;
        Ok(meta.size as u64)
    }
}
