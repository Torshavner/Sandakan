use std::sync::Arc;

use crate::application::ports::{FileLoader, LlmClient, VectorStore};
use crate::application::services::{IngestionService, RetrievalService};
use crate::presentation::config::ScaffoldConfig;

pub struct AppState<F, L, V>
where
    F: FileLoader,
    L: LlmClient,
    V: VectorStore,
{
    pub ingestion_service: Arc<IngestionService<F, L, V>>,
    pub retrieval_service: Arc<RetrievalService<L, V>>,
    pub scaffold_config: ScaffoldConfig,
}

impl<F, L, V> Clone for AppState<F, L, V>
where
    F: FileLoader,
    L: LlmClient,
    V: VectorStore,
{
    fn clone(&self) -> Self {
        Self {
            ingestion_service: Arc::clone(&self.ingestion_service),
            retrieval_service: Arc::clone(&self.retrieval_service),
            scaffold_config: self.scaffold_config.clone(),
        }
    }
}
