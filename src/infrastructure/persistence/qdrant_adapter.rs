use async_trait::async_trait;

use crate::application::ports::{SearchResult, VectorStore, VectorStoreError};
use crate::domain::{Chunk, ChunkId, Embedding};

pub struct QdrantAdapter {
    _collection_name: String,
    // qdrant_client: qdrant_client::Qdrant, // uncomment when integrating
}

impl QdrantAdapter {
    pub fn new(collection_name: String) -> Self {
        Self {
            _collection_name: collection_name,
        }
    }
}

#[async_trait]
impl VectorStore for QdrantAdapter {
    async fn upsert(
        &self,
        _chunks: &[Chunk],
        _embeddings: &[Embedding],
    ) -> Result<(), VectorStoreError> {
        // TODO: implement Qdrant upsert via gRPC
        Ok(())
    }

    async fn search(
        &self,
        _embedding: &Embedding,
        _top_k: usize,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        // TODO: implement Qdrant search via gRPC
        Ok(Vec::new())
    }

    async fn delete(&self, _chunk_ids: &[ChunkId]) -> Result<(), VectorStoreError> {
        // TODO: implement Qdrant delete via gRPC
        Ok(())
    }
}
