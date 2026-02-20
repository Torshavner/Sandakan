use std::sync::Arc;

use crate::application::ports::{AudioDecoder, TranscriptionEngine, TranscriptionError};

use super::azure_whisper_engine::AzureWhisperEngine;
use super::candle_whisper_engine::CandleWhisperEngine;
use super::openai_whisper_engine::OpenAiWhisperEngine;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TranscriptionProvider {
    Local,
    OpenAi,
    Azure,
}

pub struct TranscriptionEngineFactory;

impl TranscriptionEngineFactory {
    pub fn create(
        provider: TranscriptionProvider,
        model: &str,
        api_key: Option<String>,
        base_url: Option<String>,
        azure_deployment: Option<String>,
        azure_api_version: Option<String>,
        decoder: Option<Arc<dyn AudioDecoder>>,
    ) -> Result<Arc<dyn TranscriptionEngine>, TranscriptionError> {
        match provider {
            TranscriptionProvider::Local => {
                let decoder = decoder.ok_or_else(|| {
                    TranscriptionError::ModelLoadFailed(
                        "AudioDecoder is required for the Local transcription provider".to_string(),
                    )
                })?;
                let engine = CandleWhisperEngine::new(model, decoder)?;
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
            TranscriptionProvider::Azure => {
                let base = base_url.ok_or_else(|| {
                    TranscriptionError::ModelLoadFailed(
                        "azure_endpoint (base_url) is required for the Azure Whisper provider"
                            .to_string(),
                    )
                })?;
                let deployment = azure_deployment.ok_or_else(|| {
                    TranscriptionError::ModelLoadFailed(
                        "azure_deployment is required for the Azure Whisper provider".to_string(),
                    )
                })?;
                let key = api_key.ok_or_else(|| {
                    TranscriptionError::ModelLoadFailed(
                        "azure_key (api_key) is required for the Azure Whisper provider"
                            .to_string(),
                    )
                })?;
                let api_version = azure_api_version
                    .as_deref()
                    .unwrap_or("2024-02-01")
                    .to_string();
                let engine = AzureWhisperEngine::new(&base, &deployment, &key, &api_version);
                Ok(Arc::new(engine))
            }
        }
    }
}
