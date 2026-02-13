use std::sync::Arc;

use crate::application::ports::{
    Embedder, EmbedderError, FileLoader, FileLoaderError, JobRepository, RepositoryError,
    TextSplitter, TextSplitterError, VectorStore, VectorStoreError,
};
use crate::domain::{ContentType, Document, DocumentId, Job, JobStatus};

pub struct IngestionService<F, V, T: ?Sized>
where
    F: FileLoader,
    V: VectorStore,
    T: TextSplitter,
{
    file_loader: Arc<F>,
    embedder: Arc<dyn Embedder>,
    vector_store: Arc<V>,
    text_splitter: Arc<T>,
    job_repository: Arc<dyn JobRepository>,
}

impl<F, V, T: ?Sized> IngestionService<F, V, T>
where
    F: FileLoader,
    V: VectorStore,
    T: TextSplitter,
{
    pub fn new(
        file_loader: Arc<F>,
        embedder: Arc<dyn Embedder>,
        vector_store: Arc<V>,
        text_splitter: Arc<T>,
        job_repository: Arc<dyn JobRepository>,
    ) -> Self {
        Self {
            file_loader,
            embedder,
            vector_store,
            text_splitter,
            job_repository,
        }
    }

    pub async fn ingest(
        &self,
        data: &[u8],
        filename: String,
        content_type: ContentType,
    ) -> Result<DocumentId, IngestionError> {
        let document = Document::new(filename, content_type, data.len() as u64);
        let doc_id = document.id;

        let job = Job::new(Some(doc_id), "document_ingestion".to_string());
        let job_id = job.id;

        self.job_repository
            .create(&job)
            .await
            .map_err(IngestionError::Repository)?;

        self.job_repository
            .update_status(job_id, JobStatus::Processing, None)
            .await
            .map_err(IngestionError::Repository)?;

        let result: Result<DocumentId, IngestionError> = async {
            let text = self
                .file_loader
                .extract_text(data, &document)
                .await
                .map_err(IngestionError::FileLoading)?;

            let chunks = self
                .text_splitter
                .split(&text, doc_id)
                .await
                .map_err(IngestionError::Splitting)?;

            if chunks.is_empty() {
                return Ok(doc_id);
            }

            let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
            let embeddings = self
                .embedder
                .embed_batch(&texts)
                .await
                .map_err(IngestionError::Embedding)?;

            self.vector_store
                .upsert(&chunks, &embeddings)
                .await
                .map_err(IngestionError::Storage)?;

            Ok(doc_id)
        }
        .await;

        match &result {
            Ok(_) => {
                self.job_repository
                    .update_status(job_id, JobStatus::Completed, None)
                    .await
                    .map_err(IngestionError::Repository)?;
            }
            Err(e) => {
                let error_msg = e.to_string();
                self.job_repository
                    .update_status(job_id, JobStatus::Failed, Some(&error_msg))
                    .await
                    .map_err(IngestionError::Repository)?;
            }
        }

        result
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IngestionError {
    #[error("file loading: {0}")]
    FileLoading(FileLoaderError),
    #[error("text splitting: {0}")]
    Splitting(TextSplitterError),
    #[error("embedding: {0}")]
    Embedding(EmbedderError),
    #[error("storage: {0}")]
    Storage(VectorStoreError),
    #[error("repository: {0}")]
    Repository(RepositoryError),
}
