use std::sync::Arc;

use crate::application::ports::{
    Embedder, EmbedderError, LlmClient, LlmClientError, VectorStore, VectorStoreError,
};
use crate::application::services::count_tokens;

pub struct RetrievalService<L, V>
where
    L: LlmClient,
    V: VectorStore,
{
    embedder: Arc<dyn Embedder>,
    llm_client: Arc<L>,
    vector_store: Arc<V>,
    top_k: usize,
    similarity_threshold: f32,
    max_context_tokens: usize,
    fallback_message: String,
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
        similarity_threshold: f32,
        max_context_tokens: usize,
        fallback_message: String,
    ) -> Self {
        Self {
            embedder,
            llm_client,
            vector_store,
            top_k,
            similarity_threshold,
            max_context_tokens,
            fallback_message,
        }
    }

    #[tracing::instrument(skip(self, question), fields(retrieved_chunks_count, similarity_score))]
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

        if results.is_empty()
            || results
                .first()
                .map(|r| r.score < self.similarity_threshold)
                .unwrap_or(false)
        {
            tracing::Span::current().record("retrieved_chunks_count", 0);
            tracing::Span::current().record(
                "similarity_score",
                results.first().map(|r| r.score).unwrap_or(0.0),
            );
            return Ok(QueryResponse {
                answer: self.fallback_message.clone(),
                sources: Vec::new(),
            });
        }

        let filtered_results: Vec<_> = results
            .into_iter()
            .filter(|r| r.score >= self.similarity_threshold)
            .collect();

        let mut accumulated_tokens = 0;
        let mut trimmed_chunks = Vec::new();

        for result in &filtered_results {
            let chunk_tokens = count_tokens(&result.chunk.text);
            if accumulated_tokens + chunk_tokens <= self.max_context_tokens {
                accumulated_tokens += chunk_tokens;
                trimmed_chunks.push(result);
            } else {
                break;
            }
        }

        tracing::Span::current().record("retrieved_chunks_count", trimmed_chunks.len());
        tracing::Span::current().record(
            "similarity_score",
            trimmed_chunks.first().map(|r| r.score).unwrap_or(0.0),
        );

        let context = trimmed_chunks
            .iter()
            .map(|r| r.chunk.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let answer = self
            .llm_client
            .complete(question, &context)
            .await
            .map_err(RetrievalError::Completion)?;

        let sources = trimmed_chunks
            .into_iter()
            .map(|r| SourceChunk {
                text: r.chunk.text.clone(),
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
