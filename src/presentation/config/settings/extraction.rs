use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractorProvider {
    LocalVlm,
    LmStudio,
    Azure,
}

fn default_extractor_provider() -> ExtractorProvider {
    ExtractorProvider::LocalVlm
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionProviderSetting {
    Local,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "azure")]
    Azure,
}

fn default_transcription_provider() -> TranscriptionProviderSetting {
    TranscriptionProviderSetting::Local
}

#[derive(Debug, Clone, Deserialize)]
pub struct PdfExtractionSettings {
    pub enabled: bool,
    pub max_file_size_mb: usize,
    #[serde(default = "default_extractor_provider")]
    pub provider: ExtractorProvider,
    #[serde(default)]
    pub vlm_model: Option<String>,
    #[serde(default)]
    pub vlm_revision: Option<String>,
    #[serde(default)]
    pub vlm_base_url: Option<String>,
    #[serde(default)]
    pub vlm_api_key: Option<String>,
    #[serde(default)]
    pub azure_endpoint: Option<String>,
    #[serde(default)]
    pub azure_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AudioExtractionSettings {
    pub enabled: bool,
    pub max_file_size_mb: usize,
    pub whisper_model: String,
    #[serde(default = "default_transcription_provider")]
    pub provider: TranscriptionProviderSetting,
    #[serde(default)]
    pub azure_endpoint: Option<String>,
    #[serde(default)]
    pub azure_deployment: Option<String>,
    #[serde(default)]
    pub azure_key: Option<String>,
    #[serde(default)]
    pub azure_api_version: Option<String>,
    /// Phonetic correction map applied after local (Candle) transcription.
    /// Keys are ASR artifacts (case-insensitive match), values are the correct terms.
    #[serde(default)]
    pub asr_corrections: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VideoExtractionSettings {
    pub enabled: bool,
    pub max_file_size_mb: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractionSettings {
    pub pdf: PdfExtractionSettings,
    pub audio: AudioExtractionSettings,
    pub video: VideoExtractionSettings,
}
