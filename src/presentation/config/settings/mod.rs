mod agent;
mod chunking;
mod database;
mod embeddings;
mod eval;
mod extraction;
mod llm;
mod logging;
mod qdrant;
mod rag;
mod server;
mod storage;

pub use agent::{
    AgentServiceConfig, AgentSettings, ChatMode, FsConfig, McpSseConfig, McpStdioConfig,
    NotificationConfig, NotificationFormat, ReflectionSettings, ToolConfig, WebSearchConfig,
};
pub use chunking::{ChunkingSettings, ChunkingStrategy};
pub use database::DatabaseSettings;
pub use embeddings::{EmbeddingProvider, EmbeddingsSettings};
pub use eval::EvalSettings;
pub use extraction::{
    AudioExtractionSettings, ExtractionSettings, ExtractorProvider, PdfExtractionSettings,
    TranscriptionProviderSetting, VideoExtractionSettings,
};
pub use llm::LlmSettings;
pub use logging::LoggingSettings;
pub use qdrant::QdrantSettings;
pub use rag::RagSettings;
pub use server::ServerSettings;
pub use storage::{StorageProviderSetting, StorageSettings};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub server: ServerSettings,
    pub qdrant: QdrantSettings,
    pub database: DatabaseSettings,
    pub embeddings: EmbeddingsSettings,
    pub chunking: ChunkingSettings,
    pub llm: LlmSettings,
    pub logging: LoggingSettings,
    pub extraction: ExtractionSettings,
    pub storage: StorageSettings,
    pub rag: RagSettings,
    #[serde(default)]
    pub eval: EvalSettings,
    #[serde(default)]
    pub agent: AgentSettings,
}
