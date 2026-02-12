use async_trait::async_trait;

use crate::application::ports::{LlmClient, LlmClientError};

pub struct OpenAiClient {
    _api_key: String,
    _completion_model: String,
}

impl OpenAiClient {
    pub fn new(api_key: String, completion_model: String) -> Self {
        Self {
            _api_key: api_key,
            _completion_model: completion_model,
        }
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn complete(&self, prompt: &str, _context: &str) -> Result<String, LlmClientError> {
        // TODO: implement OpenAI chat completions API call
        Ok(format!(
            "Hello from RAG Pipeline! You asked: \"{}\". This is a stub response to verify Open WebUI connectivity.",
            prompt
        ))
    }
}
