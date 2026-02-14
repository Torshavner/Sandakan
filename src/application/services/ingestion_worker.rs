use std::sync::Arc;

use tokio::sync::mpsc;

use crate::application::ports::{
    Embedder, FileLoader, JobRepository, TextSplitter, TranscriptionEngine, VectorStore,
};
use crate::domain::{ContentType, Document, DocumentId, JobId, JobStatus};

pub struct IngestionMessage {
    pub job_id: JobId,
    pub document: Document,
    pub data: Vec<u8>,
}

pub struct IngestionWorker<F, V, T: ?Sized> {
    receiver: mpsc::Receiver<IngestionMessage>,
    file_loader: Arc<F>,
    embedder: Arc<dyn Embedder>,
    vector_store: Arc<V>,
    text_splitter: Arc<T>,
    job_repository: Arc<dyn JobRepository>,
    transcription_engine: Arc<dyn TranscriptionEngine>,
}

impl<F, V, T: ?Sized> IngestionWorker<F, V, T>
where
    F: FileLoader + 'static,
    V: VectorStore + 'static,
    T: TextSplitter + 'static,
{
    pub fn new(
        receiver: mpsc::Receiver<IngestionMessage>,
        file_loader: Arc<F>,
        embedder: Arc<dyn Embedder>,
        vector_store: Arc<V>,
        text_splitter: Arc<T>,
        job_repository: Arc<dyn JobRepository>,
        transcription_engine: Arc<dyn TranscriptionEngine>,
    ) -> Self {
        Self {
            receiver,
            file_loader,
            embedder,
            vector_store,
            text_splitter,
            job_repository,
            transcription_engine,
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
            .process_pipeline(job_id, &msg.document, &msg.data, content_type)
            .await;

        match &result {
            Ok(_) => {
                self.update_status(job_id, JobStatus::Completed, None)
                    .await?;
                tracing::info!(document_id = %doc_id.as_uuid(), "Ingestion completed");
            }
            Err(e) => {
                let error_msg = e.to_string();
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
        data: &[u8],
        content_type: ContentType,
    ) -> Result<DocumentId, IngestionWorkerError> {
        let doc_id = document.id;

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
                    .transcribe(data)
                    .await
                    .map_err(IngestionWorkerError::Transcription)?
            }
            _ => {
                self.update_status(job_id, JobStatus::Processing, None)
                    .await?;
                self.file_loader
                    .extract_text(data, document)
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
            .map_err(IngestionWorkerError::Storage)?;

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
    #[error("storage: {0}")]
    Storage(crate::application::ports::VectorStoreError),
    #[error("repository: {0}")]
    Repository(crate::application::ports::RepositoryError),
}
