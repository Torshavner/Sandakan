use async_trait::async_trait;

use super::EmbedderError;
use crate::domain::SparseEmbedding;

#[async_trait]
pub trait SparseEmbedder: Send + Sync {
    async fn embed_sparse(&self, text: &str) -> Result<SparseEmbedding, EmbedderError>;
    async fn embed_sparse_batch(
        &self,
        texts: &[&str],
    ) -> Result<Vec<SparseEmbedding>, EmbedderError>;
}
