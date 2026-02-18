use crate::application::ports::{
    CollectionConfig, SearchResult, VectorStore, VectorStoreError,
};
use crate::domain::{Chunk, ChunkId, DocumentId, Embedding};

pub struct MockVectorStore;

#[async_trait::async_trait]
impl VectorStore for MockVectorStore {
    async fn create_collection(
        &self,
        _config: &CollectionConfig,
    ) -> Result<bool, VectorStoreError> {
        Ok(true)
    }
    async fn collection_exists(&self) -> Result<bool, VectorStoreError> {
        Ok(true)
    }
    async fn get_collection_vector_size(&self) -> Result<Option<u64>, VectorStoreError> {
        Ok(Some(384))
    }
    async fn delete_collection(&self) -> Result<(), VectorStoreError> {
        Ok(())
    }
    async fn upsert(
        &self,
        _chunks: &[Chunk],
        _embeddings: &[Embedding],
    ) -> Result<(), VectorStoreError> {
        Ok(())
    }
    async fn search(
        &self,
        _embedding: &Embedding,
        _top_k: usize,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        Ok(vec![SearchResult {
            chunk: Chunk::new(
                "Rust is a systems programming language focused on safety and performance."
                    .to_string(),
                DocumentId::new(),
                Some(1),
                0,
            ),
            score: 0.95,
        }])
    }
    async fn delete(&self, _chunk_ids: &[ChunkId]) -> Result<(), VectorStoreError> {
        Ok(())
    }
}

pub struct MockVectorStoreLowScore;

#[async_trait::async_trait]
impl VectorStore for MockVectorStoreLowScore {
    async fn create_collection(
        &self,
        _config: &CollectionConfig,
    ) -> Result<bool, VectorStoreError> {
        Ok(true)
    }

    async fn collection_exists(&self) -> Result<bool, VectorStoreError> {
        Ok(true)
    }

    async fn get_collection_vector_size(&self) -> Result<Option<u64>, VectorStoreError> {
        Ok(Some(384))
    }

    async fn delete_collection(&self) -> Result<(), VectorStoreError> {
        Ok(())
    }

    async fn upsert(
        &self,
        _chunks: &[Chunk],
        _embeddings: &[Embedding],
    ) -> Result<(), VectorStoreError> {
        Ok(())
    }

    async fn search(
        &self,
        _embedding: &Embedding,
        _top_k: usize,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        Ok(vec![SearchResult {
            chunk: Chunk::new("test chunk".to_string(), DocumentId::new(), Some(1), 0),
            score: 0.3,
        }])
    }

    async fn delete(&self, _chunk_ids: &[ChunkId]) -> Result<(), VectorStoreError> {
        Ok(())
    }
}
