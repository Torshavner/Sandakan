use std::sync::Arc;

use tokio::sync::mpsc;

use crate::application::ports::{
    ConversationRepository, FileLoader, JobRepository, LlmClient, TextSplitter, VectorStore,
};
use crate::application::services::{IngestionMessage, IngestionService, RetrievalService};
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
    pub job_repository: Arc<dyn JobRepository>,
    pub ingestion_sender: mpsc::Sender<IngestionMessage>,
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
            job_repository: Arc::clone(&self.job_repository),
            ingestion_sender: self.ingestion_sender.clone(),
            settings: self.settings.clone(),
            scaffold_config: self.scaffold_config.clone(),
        }
    }
}
