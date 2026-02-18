use async_trait::async_trait;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self as m, Config};
use hf_hub::api::sync::Api;
use hf_hub::{Repo, RepoType};
use tokenizers::Tokenizer;
use tokio::sync::Mutex;

use crate::application::ports::{TranscriptionEngine, TranscriptionError};

use super::audio_decoder::decode_audio_to_pcm;

pub struct CandleWhisperEngine {
    model: Mutex<m::model::Whisper>,
    tokenizer: Tokenizer,
    config: Config,
    device: Device,
    mel_filters: Vec<f32>,
}

impl CandleWhisperEngine {
    pub fn new(model_id: &str) -> Result<Self, TranscriptionError> {
        let device = Device::Cpu;

        tracing::info!(
            device = ?device,
            model = model_id,
            "Initializing Candle Whisper transcription engine"
        );

        let api = Api::new().map_err(|e| TranscriptionError::ModelLoadFailed(e.to_string()))?;
        let repo = api.repo(Repo::new(model_id.to_string(), RepoType::Model));

        let config_path = repo
            .get("config.json")
            .map_err(|e| TranscriptionError::ModelLoadFailed(format!("config.json: {}", e)))?;
        let tokenizer_path = repo
            .get("tokenizer.json")
            .map_err(|e| TranscriptionError::ModelLoadFailed(format!("tokenizer.json: {}", e)))?;
        let weights_path = repo.get("model.safetensors").map_err(|e| {
            TranscriptionError::ModelLoadFailed(format!("model.safetensors: {}", e))
        })?;

        let mel_repo = api.repo(Repo::new(
            "FL33TW00D-HF/whisper-base".to_string(),
            RepoType::Model,
        ));
        let mel_bytes_path = mel_repo
            .get("melfilters.bytes")
            .map_err(|e| TranscriptionError::ModelLoadFailed(format!("melfilters.bytes: {}", e)))?;

        let config_contents = std::fs::read_to_string(&config_path)
            .map_err(|e| TranscriptionError::ModelLoadFailed(format!("read config: {}", e)))?;
        let config: Config = serde_json::from_str(&config_contents)
            .map_err(|e| TranscriptionError::ModelLoadFailed(format!("parse config: {}", e)))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| TranscriptionError::ModelLoadFailed(format!("tokenizer: {}", e)))?;

        let mel_bytes = std::fs::read(&mel_bytes_path)
            .map_err(|e| TranscriptionError::ModelLoadFailed(format!("mel filters: {}", e)))?;
        let mel_filters = read_mel_filters(&mel_bytes, &config)?;

        // SAFETY: safetensors files are memory-mapped read-only
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], m::DTYPE, &device)
                .map_err(|e| TranscriptionError::ModelLoadFailed(format!("weights: {}", e)))?
        };

        let model = m::model::Whisper::load(&vb, config.clone())
            .map_err(|e| TranscriptionError::ModelLoadFailed(format!("model: {}", e)))?;

        tracing::info!("Candle Whisper engine loaded successfully");

        Ok(Self {
            model: Mutex::new(model),
            tokenizer,
            config,
            device,
            mel_filters,
        })
    }
}

#[async_trait]
impl TranscriptionEngine for CandleWhisperEngine {
    async fn transcribe(&self, audio_data: &[u8]) -> Result<String, TranscriptionError> {
        let pcm = decode_audio_to_pcm(audio_data)?;

        let chunk_samples = m::N_SAMPLES;
        let mut segments: Vec<String> = Vec::new();

        let mut mel_tensors = Vec::new();

        for (i, chunk) in pcm.chunks(chunk_samples).enumerate() {
            let samples = if chunk.len() < chunk_samples {
                let mut padded = chunk.to_vec();
                padded.resize(chunk_samples, 0.0);
                padded
            } else {
                chunk.to_vec()
            };

            let mel_data = m::audio::pcm_to_mel(&self.config, &samples, &self.mel_filters);
            let n_mel = self.config.num_mel_bins;
            let n_frames = mel_data.len() / n_mel;

            let mel_tensor = Tensor::from_vec(mel_data, (1, n_mel, n_frames), &self.device)
                .map_err(|e| {
                    TranscriptionError::TranscriptionFailed(format!("mel tensor: {}", e))
                })?;

            mel_tensors.push((i, mel_tensor));
        }

        let mut model = self.model.lock().await;

        for (i, mel_tensor) in mel_tensors {
            tracing::debug!(segment = i, "Transcribing audio segment");
            let text = decode_segment(&mut model, &self.tokenizer, &self.device, &mel_tensor)?;
            if !text.is_empty() {
                segments.push(text);
            }
        }

        let transcript = segments.join(" ");

        tracing::info!(
            segments = segments.len(),
            chars = transcript.len(),
            "Audio transcription completed"
        );

        Ok(transcript)
    }
}

fn decode_segment(
    model: &mut m::model::Whisper,
    tokenizer: &Tokenizer,
    device: &Device,
    mel: &Tensor,
) -> Result<String, TranscriptionError> {
    let sot_token = token_id(tokenizer, m::SOT_TOKEN)?;
    let transcribe_token = token_id(tokenizer, m::TRANSCRIBE_TOKEN)?;
    let no_timestamps_token = token_id(tokenizer, m::NO_TIMESTAMPS_TOKEN)?;
    let eot_token = token_id(tokenizer, m::EOT_TOKEN)?;

    let audio_features = model
        .encoder
        .forward(mel, true)
        .map_err(|e| TranscriptionError::TranscriptionFailed(format!("encoder: {}", e)))?;

    let mut tokens = vec![sot_token, transcribe_token, no_timestamps_token];
    let max_tokens = 224;
    let mut decoded_text = String::new();

    for _ in 0..max_tokens {
        let token_tensor = Tensor::new(tokens.as_slice(), device)
            .map_err(|e| TranscriptionError::TranscriptionFailed(e.to_string()))?
            .unsqueeze(0)
            .map_err(|e| TranscriptionError::TranscriptionFailed(e.to_string()))?;

        let decoder_output = model
            .decoder
            .forward(&token_tensor, &audio_features, tokens.len() == 3)
            .map_err(|e| TranscriptionError::TranscriptionFailed(format!("decoder: {}", e)))?;

        let logits = model
            .decoder
            .final_linear(
                &decoder_output
                    .squeeze(0)
                    .map_err(|e| TranscriptionError::TranscriptionFailed(e.to_string()))?,
            )
            .map_err(|e| TranscriptionError::TranscriptionFailed(format!("linear: {}", e)))?;

        let seq_len = logits
            .dim(0)
            .map_err(|e| TranscriptionError::TranscriptionFailed(e.to_string()))?;
        let last_logits = logits
            .get(seq_len - 1)
            .map_err(|e| TranscriptionError::TranscriptionFailed(e.to_string()))?;

        let next_token = last_logits
            .argmax(0)
            .map_err(|e| TranscriptionError::TranscriptionFailed(e.to_string()))?
            .to_scalar::<u32>()
            .map_err(|e| TranscriptionError::TranscriptionFailed(e.to_string()))?;

        if next_token == eot_token {
            break;
        }

        tokens.push(next_token);

        if let Some(text) = tokenizer.id_to_token(next_token) {
            let text = text.replace("Ġ", " ").replace("▁", " ");
            decoded_text.push_str(&text);
        }
    }

    model.reset_kv_cache();

    Ok(decoded_text.trim().to_string())
}

fn token_id(tokenizer: &Tokenizer, token: &str) -> Result<u32, TranscriptionError> {
    tokenizer.token_to_id(token).ok_or_else(|| {
        TranscriptionError::TranscriptionFailed(format!("token not found: {}", token))
    })
}

fn read_mel_filters(bytes: &[u8], config: &Config) -> Result<Vec<f32>, TranscriptionError> {
    let expected_len = config.num_mel_bins * (m::N_FFT / 2 + 1);
    if bytes.len() < expected_len * 4 {
        return Err(TranscriptionError::ModelLoadFailed(format!(
            "mel filters file too small: {} bytes, expected at least {}",
            bytes.len(),
            expected_len * 4
        )));
    }

    let filters: Vec<f32> = bytes
        .chunks_exact(4)
        .take(expected_len)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    Ok(filters)
}
