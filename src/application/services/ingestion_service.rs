use std::sync::Arc;

use crate::application::ports::{
    FileLoader, FileLoaderError, LlmClient, LlmClientError, TextSplitter, TextSplitterError,
    VectorStore, VectorStoreError,
};
use crate::domain::{ContentType, Document, DocumentId};

pub struct IngestionService<F, L, V, T: ?Sized>
where
    F: FileLoader,
    L: LlmClient,
    V: VectorStore,
    T: TextSplitter,
{
    file_loader: Arc<F>,
    llm_client: Arc<L>,
    vector_store: Arc<V>,
    text_splitter: Arc<T>,
}

impl<F, L, V, T: ?Sized> IngestionService<F, L, V, T>
where
    F: FileLoader,
    L: LlmClient,
    V: VectorStore,
    T: TextSplitter,
{
    pub fn new(
        file_loader: Arc<F>,
        llm_client: Arc<L>,
        vector_store: Arc<V>,
        text_splitter: Arc<T>,
    ) -> Self {
        Self {
            file_loader,
            llm_client,
            vector_store,
            text_splitter,
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
            .llm_client
            .embed_batch(&texts)
            .await
            .map_err(IngestionError::Embedding)?;

        self.vector_store
            .upsert(&chunks, &embeddings)
            .await
            .map_err(IngestionError::Storage)?;

        Ok(doc_id)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IngestionError {
    #[error("file loading: {0}")]
    FileLoading(#[from] FileLoaderError),
    #[error("text splitting: {0}")]
    Splitting(#[from] TextSplitterError),
    #[error("embedding: {0}")]
    Embedding(#[from] LlmClientError),
    #[error("storage: {0}")]
    Storage(#[from] VectorStoreError),
}
