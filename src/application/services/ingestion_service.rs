use std::sync::Arc;

use crate::application::ports::{
    Embedder, EmbedderError, EvalEventRepository, EvalOutboxRepository, FileLoader,
    FileLoaderError, JobRepository, RepositoryError, SparseEmbedder, TextSplitter,
    TextSplitterError, VectorStore, VectorStoreError,
};
use crate::domain::{
    ContentType, Document, DocumentId, DocumentMetadata, EvalEvent, EvalOperationType, EvalSource,
    Job, JobStatus,
};

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
    sparse_embedder: Option<Arc<dyn SparseEmbedder>>,
    eval_event_repository: Option<Arc<dyn EvalEventRepository>>,
    eval_outbox_repository: Option<Arc<dyn EvalOutboxRepository>>,
    model_config: String,
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
        sparse_embedder: Option<Arc<dyn SparseEmbedder>>,
    ) -> Self {
        Self {
            file_loader,
            embedder,
            vector_store,
            text_splitter,
            markdown_splitter,
            job_repository,
            sparse_embedder,
            eval_event_repository: None,
            eval_outbox_repository: None,
            model_config: String::new(),
        }
    }

    pub fn with_eval(
        mut self,
        eval_event_repository: Arc<dyn EvalEventRepository>,
        eval_outbox_repository: Arc<dyn EvalOutboxRepository>,
        model_config: &str,
    ) -> Self {
        self.eval_event_repository = Some(eval_event_repository);
        self.eval_outbox_repository = Some(eval_outbox_repository);
        self.model_config = model_config.to_string();
        self
    }

    pub async fn ingest(
        &self,
        data: &[u8],
        filename: String,
        content_type: ContentType,
    ) -> Result<DocumentId, IngestionError> {
        let eval_filename = filename.clone();
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

        const MAX_EVAL_CHUNK_SAMPLES: usize = 5;

        let result: Result<(DocumentId, Vec<EvalSource>), IngestionError> = async {
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
                return Ok((doc_id, vec![]));
            }

            let chunk_samples: Vec<EvalSource> = chunks
                .iter()
                .take(MAX_EVAL_CHUNK_SAMPLES)
                .map(|c| EvalSource {
                    text: c.text.clone(),
                    page: c.page,
                    score: 0.0,
                })
                .collect();

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

            if let Some(sparse) = &self.sparse_embedder {
                let sparse_embeddings = sparse
                    .embed_sparse_batch(&texts)
                    .await
                    .map_err(IngestionError::Embedding)?;
                self.vector_store
                    .upsert_hybrid(&chunks, &embeddings, &sparse_embeddings)
                    .await
                    .map_err(IngestionError::Storage)?;
            } else {
                self.vector_store
                    .upsert(&chunks, &embeddings)
                    .await
                    .map_err(IngestionError::Storage)?;
            }

            Ok((doc_id, chunk_samples))
        }
        .await;

        match &result {
            Ok((_, chunk_samples)) => {
                self.job_repository
                    .update_status(job_id, JobStatus::Completed, None)
                    .await
                    .map_err(IngestionError::Repository)?;
                self.fire_and_forget_eval(
                    content_type,
                    &eval_filename,
                    chunk_samples.len(),
                    chunk_samples.clone(),
                );
            }
            Err(e) => {
                let error_msg = e.to_string();
                self.job_repository
                    .update_status(job_id, JobStatus::Failed, Some(&error_msg))
                    .await
                    .map_err(IngestionError::Repository)?;
            }
        }

        result.map(|(doc_id, _)| doc_id)
    }

    fn fire_and_forget_eval(
        &self,
        content_type: ContentType,
        filename: &str,
        chunk_count: usize,
        chunk_samples: Vec<EvalSource>,
    ) {
        if let (Some(event_repo), Some(outbox_repo)) =
            (&self.eval_event_repository, &self.eval_outbox_repository)
        {
            let op_type = match content_type {
                ContentType::Audio | ContentType::Video => EvalOperationType::IngestionMp4,
                ContentType::Pdf => EvalOperationType::IngestionPdf,
                ContentType::Text => EvalOperationType::Query,
            };
            let event = EvalEvent::new_ingestion(
                op_type,
                filename,
                chunk_count,
                &self.model_config,
                None,
                chunk_samples,
            );
            let event_repo = Arc::clone(event_repo);
            let outbox_repo = Arc::clone(outbox_repo);
            tokio::spawn(async move {
                match event_repo.record(&event).await {
                    Ok(_) => {
                        if let Err(e) = outbox_repo.enqueue(event.id).await {
                            tracing::warn!(error = %e, "Failed to enqueue ingestion eval outbox");
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "Failed to record ingestion eval event"),
                }
            });
        }
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
