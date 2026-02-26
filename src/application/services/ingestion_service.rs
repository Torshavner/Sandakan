use std::sync::Arc;

use crate::application::ports::{
    Embedder, EmbedderError, FileLoader, FileLoaderError, JobRepository, RepositoryError,
    TextSplitter, TextSplitterError, VectorStore, VectorStoreError,
};
use crate::domain::{ContentType, Document, DocumentId, DocumentMetadata, Job, JobStatus};

pub struct IngestionService<F, V>
where
    F: FileLoader,
    V: VectorStore,
{
    file_loader: Arc<F>,
    embedder: Arc<dyn Embedder>,
    vector_store: Arc<V>,
    text_splitter: Arc<dyn TextSplitter>,
    markdown_splitter: Arc<dyn TextSplitter>,
    job_repository: Arc<dyn JobRepository>,
}

impl<F, V> IngestionService<F, V>
where
    F: FileLoader,
    V: VectorStore,
{
    pub fn new(
        file_loader: Arc<F>,
        embedder: Arc<dyn Embedder>,
        vector_store: Arc<V>,
        text_splitter: Arc<dyn TextSplitter>,
        markdown_splitter: Arc<dyn TextSplitter>,
        job_repository: Arc<dyn JobRepository>,
    ) -> Self {
        Self {
            file_loader,
            embedder,
            vector_store,
            text_splitter,
            markdown_splitter,
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

            let metadata = Arc::new(DocumentMetadata::from_document(&document, None));

            let splitter = match content_type {
                ContentType::Pdf => &self.markdown_splitter,
                _ => &self.text_splitter,
            };

            let chunks = splitter
                .split(&text, doc_id, Some(Arc::clone(&metadata)))
                .await
                .map_err(IngestionError::Splitting)?;

            if chunks.is_empty() {
                return Ok(doc_id);
            }

            let contextual_strings: Vec<String> =
                chunks.iter().map(|c| c.as_contextual_string()).collect();
            let texts: Vec<&str> = contextual_strings.iter().map(String::as_str).collect();

            tracing::info!(
                ingestionText = text,
                ingestionMetadata = ?metadata,
                chunks = ?chunks,
                chunksWithContextuals = ?contextual_strings,
                "Ingestion service done: text, chunking, metadata"
            );

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
