use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RagSettings {
    pub similarity_threshold: f32,
    pub max_context_tokens: usize,
    pub top_k: usize,
    pub system_prompt: String,
    pub fallback_message: String,
}
