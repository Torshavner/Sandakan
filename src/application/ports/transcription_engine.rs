use async_trait::async_trait;

#[async_trait]
pub trait TranscriptionEngine: Send + Sync {
    async fn transcribe(&self, audio_data: &[u8]) -> Result<String, TranscriptionError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TranscriptionError {
    #[error("audio decoding failed: {0}")]
    DecodingFailed(String),
    #[error("transcription failed: {0}")]
    TranscriptionFailed(String),
    #[error("unsupported audio format: {0}")]
    UnsupportedFormat(String),
    #[error("model loading failed: {0}")]
    ModelLoadFailed(String),
    #[error("api request failed: {0}")]
    ApiRequestFailed(String),
}
