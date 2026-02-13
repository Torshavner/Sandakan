use std::sync::Arc;

use crate::application::ports::{
    ConversationRepository, FileLoader, LlmClient, TextSplitter, VectorStore,
};
use crate::application::services::{IngestionService, RetrievalService};
use crate::presentation::config::{ScaffoldConfig, Settings};

pub struct AppState<F, L, V, T: ?Sized>
where
    F: FileLoader,
    L: LlmClient,
    V: VectorStore,
    T: TextSplitter,
{
    pub ingestion_service: Arc<IngestionService<F, V, T>>,
    pub retrieval_service: Arc<RetrievalService<L, V>>,
    pub conversation_repository: Arc<dyn ConversationRepository>,
    pub settings: Settings,
    pub scaffold_config: ScaffoldConfig,
}

impl<F, L, V, T: ?Sized> Clone for AppState<F, L, V, T>
where
    F: FileLoader,
    L: LlmClient,
    V: VectorStore,
    T: TextSplitter,
{
    fn clone(&self) -> Self {
        Self {
            ingestion_service: Arc::clone(&self.ingestion_service),
            retrieval_service: Arc::clone(&self.retrieval_service),
            conversation_repository: Arc::clone(&self.conversation_repository),
            settings: self.settings.clone(),
            scaffold_config: self.scaffold_config.clone(),
        }
    }
}
