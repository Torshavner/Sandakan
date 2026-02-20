use async_trait::async_trait;
use reqwest::multipart;
use serde::Deserialize;

use crate::application::ports::{TranscriptionEngine, TranscriptionError};

pub struct AzureWhisperEngine {
    client: reqwest::Client,
    endpoint: String,
    api_key: String,
}

impl AzureWhisperEngine {
    pub fn new(base_url: &str, deployment: &str, api_key: &str, api_version: &str) -> Self {
        let endpoint = format!(
            "{}/openai/deployments/{}/audio/transcriptions?api-version={}",
            base_url.trim_end_matches('/'),
            deployment,
            api_version,
        );
        Self {
            client: reqwest::Client::new(),
            endpoint,
            api_key: api_key.to_string(),
        }
    }
}

#[derive(Deserialize)]
struct AzureTranscriptionResponse {
    text: String,
}

#[async_trait]
impl TranscriptionEngine for AzureWhisperEngine {
    async fn transcribe(&self, audio_data: &[u8]) -> Result<String, TranscriptionError> {
        let file_part = multipart::Part::bytes(audio_data.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| TranscriptionError::ApiRequestFailed(format!("mime: {}", e)))?;

        let form = multipart::Form::new().part("file", file_part);

        tracing::debug!(endpoint = %self.endpoint, "Sending audio to Azure OpenAI Whisper");

        let response = self
            .client
            .post(&self.endpoint)
            .header("api-key", &self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| TranscriptionError::ApiRequestFailed(format!("request: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(TranscriptionError::ApiRequestFailed(format!(
                "status {}: {}",
                status, body
            )));
        }

        let result: AzureTranscriptionResponse = response
            .json()
            .await
            .map_err(|e| TranscriptionError::ApiRequestFailed(format!("parse response: {}", e)))?;

        tracing::info!(
            chars = result.text.len(),
            "Azure OpenAI Whisper transcription completed"
        );

        Ok(result.text.trim().to_string())
    }
}
