use async_trait::async_trait;

use crate::domain::{Chunk, ChunkId, Embedding};

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn upsert(
        &self,
        chunks: &[Chunk],
        embeddings: &[Embedding],
    ) -> Result<(), VectorStoreError>;
    async fn search(
        &self,
        embedding: &Embedding,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, VectorStoreError>;
    async fn delete(&self, chunk_ids: &[ChunkId]) -> Result<(), VectorStoreError>;
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk: Chunk,
    pub score: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum VectorStoreError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("upsert failed: {0}")]
    UpsertFailed(String),
    #[error("search failed: {0}")]
    SearchFailed(String),
    #[error("delete failed: {0}")]
    DeleteFailed(String),
}
