use std::sync::Arc;

use tokio::sync::mpsc;

use crate::application::ports::{
    Embedder, FileLoader, JobRepository, StagingStore, TextSplitter, TranscriptionEngine,
    VectorStore,
};
use crate::domain::{ContentType, Document, DocumentId, JobId, JobStatus, StoragePath};

pub struct IngestionMessage {
    pub job_id: JobId,
    pub document: Document,
    pub storage_path: StoragePath,
    pub delete_after_processing: bool,
}

pub struct IngestionWorker<F, V, T: ?Sized> {
    receiver: mpsc::Receiver<IngestionMessage>,
    file_loader: Arc<F>,
    embedder: Arc<dyn Embedder>,
    vector_store: Arc<V>,
    text_splitter: Arc<T>,
    job_repository: Arc<dyn JobRepository>,
    transcription_engine: Arc<dyn TranscriptionEngine>,
    staging_store: Arc<dyn StagingStore>,
}

impl<F, V, T: ?Sized> IngestionWorker<F, V, T>
where
    F: FileLoader + 'static,
    V: VectorStore + 'static,
    T: TextSplitter + 'static,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        receiver: mpsc::Receiver<IngestionMessage>,
        file_loader: Arc<F>,
        embedder: Arc<dyn Embedder>,
        vector_store: Arc<V>,
        text_splitter: Arc<T>,
        job_repository: Arc<dyn JobRepository>,
        transcription_engine: Arc<dyn TranscriptionEngine>,
        staging_store: Arc<dyn StagingStore>,
    ) -> Self {
        Self {
            receiver,
            file_loader,
            embedder,
            vector_store,
            text_splitter,
            job_repository,
            transcription_engine,
            staging_store,
        }
    }

    pub async fn run(mut self) {
        tracing::info!("Ingestion worker started");
        while let Some(msg) = self.receiver.recv().await {
            let span = tracing::info_span!(
                "ingestion_job",
                job_id = %msg.job_id.as_uuid(),
                document_id = %msg.document.id.as_uuid(),
                filename = %msg.document.filename,
            );
            let _guard = span.enter();

            if let Err(e) = self.process_job(msg).await {
                tracing::error!(error = %e, "Ingestion job failed");
            }
        }
        tracing::info!("Ingestion worker stopped: channel closed");
    }

    async fn process_job(&self, msg: IngestionMessage) -> Result<(), IngestionWorkerError> {
        let job_id = msg.job_id;
        let doc_id = msg.document.id;
        let content_type = msg.document.content_type;

        self.update_status(job_id, JobStatus::Processing, None)
            .await?;

        let result = self
            .process_pipeline(job_id, &msg.document, &msg.storage_path, content_type)
            .await;

        match &result {
            Ok(_) => {
                if msg.delete_after_processing {
                    if let Err(e) = self.staging_store.delete(&msg.storage_path).await {
                        tracing::warn!(
                            error = %e,
                            path = %msg.storage_path,
                            "Failed to delete staged file after successful ingestion"
                        );
                    }
                }
                self.update_status(job_id, JobStatus::Completed, None)
                    .await?;
                tracing::info!(document_id = %doc_id.as_uuid(), "Ingestion completed");
            }
            Err(e) => {
                let error_msg = e.to_string();
                if msg.delete_after_processing {
                    if let Err(del_err) = self.staging_store.delete(&msg.storage_path).await {
                        tracing::warn!(
                            error = %del_err,
                            path = %msg.storage_path,
                            "Failed to delete staged file after job failure"
                        );
                    }
                }
                self.update_status(job_id, JobStatus::Failed, Some(&error_msg))
                    .await?;
            }
        }

        result.map(|_| ())
    }

    async fn process_pipeline(
        &self,
        job_id: JobId,
        document: &Document,
        storage_path: &StoragePath,
        content_type: ContentType,
    ) -> Result<DocumentId, IngestionWorkerError> {
        let doc_id = document.id;

        let data = self
            .staging_store
            .fetch(storage_path)
            .await
            .map_err(IngestionWorkerError::Staging)?;

        let text = match content_type {
            ContentType::Audio | ContentType::Video => {
                self.update_status(job_id, JobStatus::MediaExtraction, None)
                    .await?;
                tracing::debug!(
                    content_type = ?content_type,
                    "Starting media extraction"
                );

                self.update_status(job_id, JobStatus::Transcribing, None)
                    .await?;
                tracing::debug!("Starting audio transcription");

                self.transcription_engine
                    .transcribe(&data)
                    .await
                    .map_err(IngestionWorkerError::Transcription)?
            }
            _ => {
                self.update_status(job_id, JobStatus::Processing, None)
                    .await?;
                self.file_loader
                    .extract_text(&data, document)
                    .await
                    .map_err(IngestionWorkerError::FileLoading)?
            }
        };

        self.update_status(job_id, JobStatus::Embedding, None)
            .await?;

        let chunks = self
            .text_splitter
            .split(&text, doc_id)
            .await
            .map_err(IngestionWorkerError::Splitting)?;

        if chunks.is_empty() {
            return Ok(doc_id);
        }

        let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
        let embeddings = self
            .embedder
            .embed_batch(&texts)
            .await
            .map_err(IngestionWorkerError::Embedding)?;

        self.vector_store
            .upsert(&chunks, &embeddings)
            .await
            .map_err(IngestionWorkerError::VectorStore)?;

        Ok(doc_id)
    }

    async fn update_status(
        &self,
        job_id: JobId,
        status: JobStatus,
        error_message: Option<&str>,
    ) -> Result<(), IngestionWorkerError> {
        tracing::debug!(status = %status, "Job status transition");
        self.job_repository
            .update_status(job_id, status, error_message)
            .await
            .map_err(IngestionWorkerError::Repository)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IngestionWorkerError {
    #[error("file loading: {0}")]
    FileLoading(crate::application::ports::FileLoaderError),
    #[error("transcription: {0}")]
    Transcription(crate::application::ports::TranscriptionError),
    #[error("text splitting: {0}")]
    Splitting(crate::application::ports::TextSplitterError),
    #[error("embedding: {0}")]
    Embedding(crate::application::ports::EmbedderError),
    #[error("vector store: {0}")]
    VectorStore(crate::application::ports::VectorStoreError),
    #[error("repository: {0}")]
    Repository(crate::application::ports::RepositoryError),
    #[error("staging store: {0}")]
    Staging(crate::application::ports::StagingStoreError),
}
