use async_trait::async_trait;

use super::{CollectionConfig, SearchResult, VectorStoreError};
use crate::domain::{Chunk, ChunkId, Embedding};

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn create_collection(&self, config: &CollectionConfig) -> Result<bool, VectorStoreError>;

    async fn collection_exists(&self) -> Result<bool, VectorStoreError>;

    async fn delete_collection(&self) -> Result<(), VectorStoreError>;

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
