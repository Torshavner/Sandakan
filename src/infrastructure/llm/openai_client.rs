use async_trait::async_trait;

use crate::application::ports::{LlmClient, LlmClientError};
use crate::domain::Embedding;

pub struct OpenAiClient {
    _api_key: String,
    _embedding_model: String,
    _completion_model: String,
}

impl OpenAiClient {
    pub fn new(api_key: String, embedding_model: String, completion_model: String) -> Self {
        Self {
            _api_key: api_key,
            _embedding_model: embedding_model,
            _completion_model: completion_model,
        }
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn embed(&self, _text: &str) -> Result<Embedding, LlmClientError> {
        // TODO: implement OpenAI embeddings API call
        Ok(Embedding::new(vec![0.0; 1536]))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>, LlmClientError> {
        // TODO: implement batch embeddings
        Ok(texts
            .iter()
            .map(|_| Embedding::new(vec![0.0; 1536]))
            .collect())
    }

    async fn complete(&self, prompt: &str, _context: &str) -> Result<String, LlmClientError> {
        Ok(format!(
            "Hello from RAG Pipeline! You asked: \"{}\". This is a stub response to verify Open WebUI connectivity.",
            prompt
        ))
    }
}
