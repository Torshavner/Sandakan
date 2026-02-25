use async_trait::async_trait;

use crate::application::ports::{EmbedderError, RepositoryError, VectorStoreError};

/// A raw source passage retrieved from the vector store.
#[derive(Debug, Clone)]
pub struct SourceChunk {
    pub text: String,
    pub page: Option<u32>,
    pub score: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum RetrievalError {
    #[error("embedding: {0}")]
    Embedding(EmbedderError),
    #[error("search: {0}")]
    Search(#[from] VectorStoreError),
    #[error("completion: {0}")]
    Completion(crate::application::ports::LlmClientError),
    #[error("repository: {0}")]
    Repository(RepositoryError),
}

/// Port for performing retrieval-only (embed + search + filter) queries against the vector store.
///
/// Deliberately omits LLM synthesis — callers (e.g. `RagSearchAdapter`) receive raw source
/// passages and produce their own synthesis, avoiding a double-LLM chain.
#[async_trait]
pub trait RetrievalServicePort: Send + Sync {
    async fn search_chunks(&self, query: &str) -> Result<Vec<SourceChunk>, RetrievalError>;
}
