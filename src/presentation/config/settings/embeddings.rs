use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingProvider {
    Local,
    #[serde(rename = "openai")]
    OpenAi,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddingsSettings {
    pub provider: EmbeddingProvider,
    pub model: String,
    pub dimension: usize,
    pub chunk_overlap: usize,
}
