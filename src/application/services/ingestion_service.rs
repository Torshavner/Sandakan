use std::sync::Arc;

use crate::application::ports::{
    Embedder, EmbedderError, FileLoader, FileLoaderError, TextSplitter, TextSplitterError,
    VectorStore, VectorStoreError,
};
use crate::domain::{ContentType, Document, DocumentId};

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
    ) -> Self {
        Self {
            file_loader,
            embedder,
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
}

#[derive(Debug, thiserror::Error)]
pub enum IngestionError {
    #[error("file loading: {0}")]
    FileLoading(#[from] FileLoaderError),
    #[error("text splitting: {0}")]
    Splitting(#[from] TextSplitterError),
    #[error("embedding: {0}")]
    Embedding(#[from] EmbedderError),
    #[error("storage: {0}")]
    Storage(#[from] VectorStoreError),
}
