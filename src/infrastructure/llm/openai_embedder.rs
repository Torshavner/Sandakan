use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::application::ports::{Embedder, EmbedderError};
use crate::domain::Embedding;

pub struct OpenAiEmbedder {
    client: Client,
    api_key: String,
    model: String,
}

#[derive(Serialize)]
struct EmbeddingRequest {
    input: Vec<String>,
    model: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

impl OpenAiEmbedder {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }
}

#[async_trait]
impl Embedder for OpenAiEmbedder {
    async fn embed(&self, text: &str) -> Result<Embedding, EmbedderError> {
        let results = self.embed_batch(&[text]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| EmbedderError::InvalidResponse("empty response".to_string()))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>, EmbedderError> {
        let request_body = EmbeddingRequest {
            input: texts.iter().map(|t| (*t).to_string()).collect(),
            model: self.model.clone(),
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| EmbedderError::ApiRequestFailed(e.to_string()))?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(EmbedderError::RateLimited);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(EmbedderError::ApiRequestFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let embedding_response: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| EmbedderError::InvalidResponse(e.to_string()))?;

        Ok(embedding_response
            .data
            .into_iter()
            .map(|d| Embedding::new(d.embedding))
            .collect())
    }
}
