use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub server: ServerSettings,
    pub qdrant: QdrantSettings,
    pub embeddings: EmbeddingsSettings,
    pub chunking: ChunkingSettings,
    pub llm: LlmSettings,
    pub logging: LoggingSettings,
    pub extraction: ExtractionSettings,
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
    pub api_key: String,
    pub chat_model: String,
    pub max_tokens: usize,
    pub temperature: f32,
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
