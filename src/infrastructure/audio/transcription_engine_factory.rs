use std::sync::Arc;

use crate::application::ports::{TranscriptionEngine, TranscriptionError};

use super::candle_whisper_engine::CandleWhisperEngine;
use super::openai_whisper_engine::OpenAiWhisperEngine;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TranscriptionProvider {
    Local,
    OpenAi,
}

pub struct TranscriptionEngineFactory;

impl TranscriptionEngineFactory {
    pub fn create(
        provider: TranscriptionProvider,
        model: &str,
        api_key: Option<String>,
        base_url: Option<String>,
    ) -> Result<Arc<dyn TranscriptionEngine>, TranscriptionError> {
        match provider {
            TranscriptionProvider::Local => {
                let engine = CandleWhisperEngine::new(model)?;
                Ok(Arc::new(engine))
            }
            TranscriptionProvider::OpenAi => {
                let key = api_key.ok_or_else(|| {
                    TranscriptionError::ModelLoadFailed(
                        "API key required for OpenAI Whisper".to_string(),
                    )
                })?;
                let engine = OpenAiWhisperEngine::new(key, base_url, Some(model.to_string()));
                Ok(Arc::new(engine))
            }
        }
    }
}
