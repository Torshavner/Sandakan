use async_trait::async_trait;

use super::{CollectionConfig, SearchResult, VectorStoreError};
use crate::domain::{Chunk, ChunkId, Embedding, SparseEmbedding};

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn create_collection(&self, config: &CollectionConfig) -> Result<bool, VectorStoreError>;

    async fn collection_exists(&self) -> Result<bool, VectorStoreError>;

    async fn get_collection_vector_size(&self) -> Result<Option<u64>, VectorStoreError>;

    async fn is_hybrid_collection(&self) -> Result<bool, VectorStoreError> {
        Ok(false)
    }

    async fn delete_collection(&self) -> Result<(), VectorStoreError>;

    async fn upsert(
        &self,
        chunks: &[Chunk],
        embeddings: &[Embedding],
    ) -> Result<(), VectorStoreError>;

    async fn upsert_hybrid(
        &self,
        chunks: &[Chunk],
        dense: &[Embedding],
        sparse: &[SparseEmbedding],
    ) -> Result<(), VectorStoreError> {
        let _ = sparse;
        self.upsert(chunks, dense).await
    }

    async fn search(
        &self,
        embedding: &Embedding,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, VectorStoreError>;

    async fn search_hybrid(
        &self,
        dense: &Embedding,
        sparse: &SparseEmbedding,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        let _ = sparse;
        self.search(dense, top_k).await
    }

    async fn delete(&self, chunk_ids: &[ChunkId]) -> Result<(), VectorStoreError>;
}
