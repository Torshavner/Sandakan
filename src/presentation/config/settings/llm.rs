use serde::Deserialize;

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
