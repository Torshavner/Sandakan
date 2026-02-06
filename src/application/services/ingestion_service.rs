use std::sync::Arc;

use crate::application::ports::{
    FileLoader, FileLoaderError, LlmClient, LlmClientError, VectorStore, VectorStoreError,
};
use crate::domain::{Chunk, ContentType, Document, DocumentId};

pub struct IngestionService<F, L, V>
where
    F: FileLoader,
    L: LlmClient,
    V: VectorStore,
{
    file_loader: Arc<F>,
    llm_client: Arc<L>,
    vector_store: Arc<V>,
    chunk_size: usize,
    chunk_overlap: usize,
}

impl<F, L, V> IngestionService<F, L, V>
where
    F: FileLoader,
    L: LlmClient,
    V: VectorStore,
{
    pub fn new(
        file_loader: Arc<F>,
        llm_client: Arc<L>,
        vector_store: Arc<V>,
        chunk_size: usize,
        chunk_overlap: usize,
    ) -> Self {
        Self {
            file_loader,
            llm_client,
            vector_store,
            chunk_size,
            chunk_overlap,
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

        let chunks = self.split_into_chunks(&text, doc_id);
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

    fn split_into_chunks(&self, text: &str, document_id: DocumentId) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let total_len = chars.len();

        if total_len == 0 {
            return chunks;
        }

        let mut offset = 0;
        while offset < total_len {
            let end = (offset + self.chunk_size).min(total_len);
            let chunk_text: String = chars[offset..end].iter().collect();

            chunks.push(Chunk::new(chunk_text, document_id, None, offset));

            let step = if self.chunk_size > self.chunk_overlap {
                self.chunk_size - self.chunk_overlap
            } else {
                self.chunk_size
            };

            offset += step;
        }

        chunks
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IngestionError {
    #[error("file loading: {0}")]
    FileLoading(#[from] FileLoaderError),
    #[error("embedding: {0}")]
    Embedding(#[from] LlmClientError),
    #[error("storage: {0}")]
    Storage(#[from] VectorStoreError),
}
