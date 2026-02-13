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
    pub rag: RagSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QdrantSettings {
    pub url: String,
    pub collection_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddingsSettings {
    pub provider: EmbeddingProvider,
    pub model: String,
    pub strategy: EmbeddingStrategy,
    pub dimension: usize,
    pub chunk_overlap: usize,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingProvider {
    Local,
    #[serde(rename = "openai")]
    OpenAi,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingStrategy {
    Semantic,
    Fixed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChunkingSettings {
    pub max_chunk_size: usize,
    pub overlap_tokens: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmSettings {
    pub provider: String,
    pub api_key: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub azure_endpoint: Option<String>,
    pub chat_model: String,
    pub max_tokens: usize,
    pub temperature: f32,
    #[serde(default = "default_sse_keep_alive")]
    pub sse_keep_alive_seconds: u64,
}

fn default_sse_keep_alive() -> u64 {
    15
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingSettings {
    pub level: String,
    pub enable_json: bool,
    pub enable_udp: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractionSettings {
    pub pdf: PdfExtractionSettings,
    pub audio: AudioExtractionSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PdfExtractionSettings {
    pub enabled: bool,
    pub max_file_size_mb: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AudioExtractionSettings {
    pub enabled: bool,
    pub max_file_size_mb: usize,
    pub whisper_model: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RagSettings {
    pub similarity_threshold: f32,
    pub max_context_tokens: usize,
    pub top_k: usize,
    pub system_prompt: String,
    pub fallback_message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSettings {
    pub url: String,
    pub max_connections: u32,
    pub run_migrations: bool,
}
