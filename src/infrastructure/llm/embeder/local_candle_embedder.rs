use async_trait::async_trait;
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig};
use hf_hub::api::sync::Api;
use hf_hub::{Repo, RepoType};
use tokenizers::Tokenizer;

use crate::application::ports::{Embedder, EmbedderError};
use crate::domain::Embedding;

pub struct LocalCandleEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl LocalCandleEmbedder {
    pub fn new(model_id: &str) -> Result<Self, EmbedderError> {
        let device = Self::select_device();

        tracing::info!(
            device = ?device,
            model = model_id,
            "Initializing local Candle embedding model"
        );

        let api = Api::new().map_err(|e| EmbedderError::ModelLoadFailed(e.to_string()))?;
        let repo = api.repo(Repo::new(model_id.to_string(), RepoType::Model));

        let config_path = repo
            .get("config.json")
            .map_err(|e| EmbedderError::ModelLoadFailed(format!("config.json: {}", e)))?;
        let tokenizer_path = repo
            .get("tokenizer.json")
            .map_err(|e| EmbedderError::ModelLoadFailed(format!("tokenizer.json: {}", e)))?;
        let weights_path = repo
            .get("model.safetensors")
            .map_err(|e| EmbedderError::ModelLoadFailed(format!("model.safetensors: {}", e)))?;

        let config_contents = std::fs::read_to_string(&config_path)
            .map_err(|e| EmbedderError::ModelLoadFailed(format!("read config: {}", e)))?;
        let config: BertConfig = serde_json::from_str(&config_contents)
            .map_err(|e| EmbedderError::ModelLoadFailed(format!("parse config: {}", e)))?;

        let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| EmbedderError::ModelLoadFailed(format!("tokenizer: {}", e)))?;

        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: config.max_position_embeddings,
                ..Default::default()
            }))
            .map_err(|e| EmbedderError::ModelLoadFailed(format!("truncation config: {}", e)))?;

        let dtype = Self::select_dtype(&device);

        // SAFETY: safetensors files are memory-mapped read-only
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], dtype, &device)
                .map_err(|e| EmbedderError::ModelLoadFailed(format!("weights: {}", e)))?
        };

        let model = BertModel::load(vb, &config)
            .map_err(|e| EmbedderError::ModelLoadFailed(format!("model: {}", e)))?;

        tracing::info!("Local Candle embedding model loaded successfully");

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    fn select_device() -> Device {
        Device::new_metal(0).unwrap_or(Device::Cpu)
    }

    pub fn select_dtype(device: &Device) -> DType {
        if device.is_cpu() {
            DType::F32
        } else {
            DType::F16
        }
    }

    fn encode_texts(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedderError> {
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| EmbedderError::InferenceFailed(format!("tokenization: {}", e)))?;

        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0);

        let mut all_input_ids = Vec::with_capacity(texts.len() * max_len);
        let mut all_type_ids = Vec::with_capacity(texts.len() * max_len);
        let mut all_attention_mask = Vec::with_capacity(texts.len() * max_len);

        for encoding in &encodings {
            let ids = encoding.get_ids();
            let type_ids = encoding.get_type_ids();
            let attention = encoding.get_attention_mask();
            let pad_len = max_len - ids.len();

            all_input_ids.extend_from_slice(ids);
            all_input_ids.extend(std::iter::repeat_n(0u32, pad_len));

            all_type_ids.extend_from_slice(type_ids);
            all_type_ids.extend(std::iter::repeat_n(0u32, pad_len));

            all_attention_mask.extend_from_slice(attention);
            all_attention_mask.extend(std::iter::repeat_n(0u32, pad_len));
        }

        let batch_size = texts.len();
        let input_ids = Tensor::from_vec(all_input_ids, (batch_size, max_len), &self.device)
            .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?;
        let token_type_ids = Tensor::from_vec(all_type_ids, (batch_size, max_len), &self.device)
            .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?;
        let attention_mask =
            Tensor::from_vec(all_attention_mask, (batch_size, max_len), &self.device)
                .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?;

        let embeddings = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attention_mask))
            .and_then(|t| t.to_dtype(DType::F32))
            .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?;

        // Mean pooling with attention mask
        let attention_mask_f32 = attention_mask
            .to_dtype(DType::F32)
            .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?;
        let attention_expanded = attention_mask_f32
            .unsqueeze(2)
            .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?;
        let masked = embeddings
            .broadcast_mul(&attention_expanded)
            .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?;
        let summed = masked
            .sum(1)
            .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?;
        let token_counts = attention_mask_f32
            .sum(1)
            .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?
            .unsqueeze(1)
            .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?;
        let pooled = summed
            .broadcast_div(&token_counts)
            .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?;

        // L2 normalization + extract per-sentence vectors
        let mut results = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let vec_i = pooled
                .get(i)
                .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?;
            let mut values: Vec<f32> = vec_i
                .to_vec1()
                .map_err(|e| EmbedderError::InferenceFailed(e.to_string()))?;
            l2_normalize(&mut values);
            results.push(values);
        }

        Ok(results)
    }
}

fn l2_normalize(v: &mut [f32]) {
    let length: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if length > 0.0 {
        v.iter_mut().for_each(|x| *x /= length);
    }
}

#[async_trait]
impl Embedder for LocalCandleEmbedder {
    async fn embed(&self, text: &str) -> Result<Embedding, EmbedderError> {
        let results = self.encode_texts(&[text])?;
        results
            .into_iter()
            .next()
            .map(Embedding::new)
            .ok_or_else(|| EmbedderError::InferenceFailed("empty result".to_string()))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>, EmbedderError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let results = self.encode_texts(texts)?;
        Ok(results.into_iter().map(Embedding::new).collect())
    }
}
