use std::sync::Arc;

use crate::application::ports::{Embedder, ToolRegistry, ToolSchema};
use crate::domain::Embedding;

/// A tool registry that uses vector similarity to return only the most
/// relevant tools for a given user intent.
///
/// At construction, each tool's description is embedded once. On lookup,
/// the query intent is embedded and compared via cosine similarity.
/// Falls back to returning all tools if the embedder fails.
pub struct SemanticToolRegistry {
    schemas: Vec<ToolSchema>,
    embeddings: Vec<Embedding>,
    embedder: Arc<dyn Embedder>,
}

impl SemanticToolRegistry {
    /// Embeds all tool descriptions at construction time.
    ///
    /// If embedding fails for any tool, the registry falls back to behaving
    /// like a static registry (search_tools returns all tools).
    pub async fn try_new(
        schemas: Vec<ToolSchema>,
        embedder: Arc<dyn Embedder>,
    ) -> Result<Self, String> {
        let descriptions: Vec<&str> = schemas.iter().map(|s| s.description.as_str()).collect();
        let embeddings = embedder
            .embed_batch(&descriptions)
            .await
            .map_err(|e| format!("failed to embed tool descriptions: {e}"))?;

        tracing::info!(
            tool_count = schemas.len(),
            "Semantic tool registry initialised"
        );

        Ok(Self {
            schemas,
            embeddings,
            embedder,
        })
    }
}

#[async_trait::async_trait]
impl ToolRegistry for SemanticToolRegistry {
    fn list_tools(&self) -> Vec<ToolSchema> {
        self.schemas.clone()
    }

    async fn search_tools(&self, intent: &str, top_k: usize) -> Vec<ToolSchema> {
        let query_embedding = match self.embedder.embed(intent).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to embed intent; returning all tools");
                return self.schemas.clone();
            }
        };

        let mut scored: Vec<(usize, f32)> = self
            .embeddings
            .iter()
            .enumerate()
            .map(|(i, emb)| (i, query_embedding.cosine_similarity(emb)))
            .collect();

        scored.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .take(top_k)
            .map(|(i, score)| {
                tracing::debug!(
                    tool = %self.schemas[i].name,
                    score = score,
                    "Semantic tool match"
                );
                self.schemas[i].clone()
            })
            .collect()
    }
}
