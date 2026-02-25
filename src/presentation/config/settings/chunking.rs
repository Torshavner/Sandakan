use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChunkingStrategy {
    Semantic,
    Fixed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChunkingSettings {
    pub max_chunk_size: usize,
    pub overlap_tokens: usize,
    pub strategy: ChunkingStrategy,
}
