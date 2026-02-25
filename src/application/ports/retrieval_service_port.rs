use async_trait::async_trait;

use crate::application::ports::{EmbedderError, RepositoryError, VectorStoreError};

/// A raw source passage retrieved from the vector store.
#[derive(Debug, Clone)]
pub struct SourceChunk {
    pub text: String,
    pub page: Option<u32>,
    pub score: f32,
    pub title: Option<String>,
    /// Base source URL for the document (without timestamp suffix).
    pub source_url: Option<String>,
    pub content_type: Option<String>,
    /// Start time of the chunk within the media file, in seconds.
    /// `None` for non-media sources (PDF, plain text).
    pub start_time: Option<f32>,
}

impl SourceChunk {
    /// Returns the source URL with an appended `?t=Xs` / `&t=Xs` timestamp suffix when
    /// `start_time` is set, enabling deep-link citations (e.g. YouTube `&t=1045s`).
    pub fn timestamped_url(&self) -> Option<String> {
        let base = self.source_url.as_deref()?;
        match self.start_time {
            Some(t) => {
                let secs = t.round() as u64;
                let separator = if base.contains('?') { '&' } else { '?' };
                Some(format!("{}{}t={}s", base, separator, secs))
            }
            None => Some(base.to_string()),
        }
    }
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
