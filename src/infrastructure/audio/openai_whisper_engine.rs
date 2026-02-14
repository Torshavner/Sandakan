use async_trait::async_trait;
use reqwest::multipart;

use crate::application::ports::{TranscriptionEngine, TranscriptionError};

pub struct OpenAiWhisperEngine {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAiWhisperEngine {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            model: model.unwrap_or_else(|| "whisper-1".to_string()),
        }
    }
}

#[async_trait]
impl TranscriptionEngine for OpenAiWhisperEngine {
    async fn transcribe(&self, audio_data: &[u8]) -> Result<String, TranscriptionError> {
        let url = format!("{}/audio/transcriptions", self.base_url);

        let file_part = multipart::Part::bytes(audio_data.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| TranscriptionError::ApiRequestFailed(format!("mime: {}", e)))?;

        let form = multipart::Form::new()
            .text("model", self.model.clone())
            .text("response_format", "text")
            .part("file", file_part);

        tracing::debug!(model = %self.model, "Sending audio to OpenAI Whisper API");

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
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

        let transcript = response
            .text()
            .await
            .map_err(|e| TranscriptionError::ApiRequestFailed(format!("body: {}", e)))?;

        tracing::info!(
            chars = transcript.len(),
            "OpenAI Whisper transcription completed"
        );

        Ok(transcript.trim().to_string())
    }
}
