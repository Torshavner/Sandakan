use std::sync::Arc;

use crate::application::ports::{
    Embedder, EmbedderError, LlmClient, LlmClientError, VectorStore, VectorStoreError,
};

pub struct RetrievalService<L, V>
where
    L: LlmClient,
    V: VectorStore,
{
    embedder: Arc<dyn Embedder>,
    llm_client: Arc<L>,
    vector_store: Arc<V>,
    top_k: usize,
}

impl<L, V> RetrievalService<L, V>
where
    L: LlmClient,
    V: VectorStore,
{
    pub fn new(
        embedder: Arc<dyn Embedder>,
        llm_client: Arc<L>,
        vector_store: Arc<V>,
        top_k: usize,
    ) -> Self {
        Self {
            embedder,
            llm_client,
            vector_store,
            top_k,
        }
    }

    pub async fn query(&self, question: &str) -> Result<QueryResponse, RetrievalError> {
        let query_embedding = self
            .embedder
            .embed(question)
            .await
            .map_err(RetrievalError::Embedding)?;

        let results = self
            .vector_store
            .search(&query_embedding, self.top_k)
            .await
            .map_err(RetrievalError::Search)?;

        if results.is_empty() {
            return Ok(QueryResponse {
                answer: "No relevant context found.".to_string(),
                sources: Vec::new(),
            });
        }

        let context = results
            .iter()
            .map(|r| r.chunk.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let answer = self
            .llm_client
            .complete(question, &context)
            .await
            .map_err(RetrievalError::Completion)?;

        let sources = results
            .into_iter()
            .map(|r| SourceChunk {
                text: r.chunk.text,
                page: r.chunk.page,
                score: r.score,
            })
            .collect();

        Ok(QueryResponse { answer, sources })
    }
}

#[derive(Debug, Clone)]
pub struct QueryResponse {
    pub answer: String,
    pub sources: Vec<SourceChunk>,
}

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
    Completion(LlmClientError),
}
