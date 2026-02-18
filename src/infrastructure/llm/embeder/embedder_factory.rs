use std::sync::Arc;

use crate::application::ports::Embedder;
use crate::presentation::config::EmbeddingProvider;

use crate::infrastructure::llm::{LocalCandleEmbedder, OpenAiEmbedder};

pub struct EmbedderFactory;

#[derive(Debug, thiserror::Error)]
pub enum EmbedderFactoryError {
    #[error("missing API key: OpenAI embedder requires OPENAI_API_KEY")]
    MissingApiKey,
    #[error("model initialization failed: {0}")]
    InitializationFailed(String),
}

impl EmbedderFactory {
    pub fn create(
        provider: EmbeddingProvider,
        model: String,
        api_key: Option<String>,
    ) -> Result<Arc<dyn Embedder>, EmbedderFactoryError> {
        match provider {
            EmbeddingProvider::Local => {
                tracing::info!(model = %model, "Loading local Candle embedding model");
                let embedder = LocalCandleEmbedder::new(&model)
                    .map_err(|e| EmbedderFactoryError::InitializationFailed(e.to_string()))?;
                Ok(Arc::new(embedder))
            }
            EmbeddingProvider::OpenAi => {
                let key = api_key
                    .filter(|k| !k.is_empty())
                    .ok_or(EmbedderFactoryError::MissingApiKey)?;
                tracing::info!(model = %model, "Loading OpenAI embedding model");
                Ok(Arc::new(OpenAiEmbedder::new(key, model)))
            }
        }
    }
}
