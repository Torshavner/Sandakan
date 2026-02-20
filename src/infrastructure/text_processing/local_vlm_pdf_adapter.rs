// @AI-BYPASS-LENGTH: single LocalVlmPdfAdapter impl block with async VLM inference loop; not splittable.
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use candle_core::{DType, Device, Module, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::moondream;
use hf_hub::api::sync::Api;
use hf_hub::{Repo, RepoType};
use image::imageops::FilterType;
use tokenizers::Tokenizer;
use tokio::sync::Mutex;

use crate::application::ports::{FileLoader, FileLoaderError};
use crate::domain::{ContentType, Document};

use super::pdf_rasterizer::rasterize_pages;
use super::text_sanitizer::sanitize_extracted_text;

pub const EXTRACTION_TIMEOUT: Duration = Duration::from_secs(300);
pub const MAX_PAGES_DUE_TO_RAM_USAGE: usize = 200;
pub const RENDER_DPI: f32 = 150.0;
pub const OCR_PROMPT: &str = "You are an expert OCR and document extraction AI for a RAG database. \
Extract all the text from this presentation slide and format it as clean, structured Markdown.\n\
\n\
Follow these strict rules:\n\
1. Preserve the visual hierarchy using appropriate Markdown headers (#, ##).\n\
2. Format all lists using standard Markdown bullet points.\n\
3. ABSOLUTELY NO ASCII ART: Do not attempt to draw charts, graphs, diagrams, or spatial layouts using characters (like |, -, or ●).\n\
4. CHARTS AND GRAPHS: If the slide contains a chart or diagram, simply list the text labels found within it and write a brief 1-2 sentence text summary of what the visual data shows.\n\
5. Output ONLY the extracted Markdown text. Do not include any conversational filler.";

const MOONDREAM_IMAGE_SIZE: u32 = 378;
const MOONDREAM_MAX_TOKENS: usize = 1024;

pub fn parse_shard_names(index_json: &str) -> Result<Vec<String>, FileLoaderError> {
    let index: serde_json::Value = serde_json::from_str(index_json)
        .map_err(|e| FileLoaderError::ExtractionFailed(format!("parse index.json: {e}")))?;

    let weight_map = index
        .get("weight_map")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            FileLoaderError::ExtractionFailed("index.json missing weight_map object".to_string())
        })?;

    let names: BTreeSet<String> = weight_map
        .values()
        .filter_map(|v| v.as_str())
        .map(str::to_owned)
        .collect();

    if names.is_empty() {
        return Err(FileLoaderError::ExtractionFailed(
            "index.json weight_map contains no shard filenames".to_string(),
        ));
    }

    Ok(names.into_iter().collect())
}

pub struct LocalVlmPdfAdapter {
    model: Arc<Mutex<moondream::Model>>,
    tokenizer: Arc<Tokenizer>,
    device: Arc<Device>,
}

impl LocalVlmPdfAdapter {
    pub fn new(model_id: &str, revision: Option<&str>) -> Result<Self, FileLoaderError> {
        let device = Device::new_metal(0).unwrap_or(Device::Cpu);

        tracing::info!(
            model = model_id,
            device = ?device,
            "Initializing local VLM PDF adapter"
        );

        let api = Api::new()
            .map_err(|e| FileLoaderError::ExtractionFailed(format!("hf-hub init failed: {e}")))?;
        let repo = api.repo(match revision {
            Some(rev) => {
                Repo::with_revision(model_id.to_string(), RepoType::Model, rev.to_string())
            }
            None => Repo::new(model_id.to_string(), RepoType::Model),
        });

        let tokenizer_path = repo
            .get("tokenizer.json")
            .map_err(|e| FileLoaderError::ExtractionFailed(format!("tokenizer.json: {e}")))?;

        let weight_paths = Self::load_weights(&repo)?;

        let dtype = if device.is_cpu() {
            DType::F32
        } else {
            DType::F16
        };

        // SAFETY: safetensors files are memory-mapped read-only from a locally cached HF repo.
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&weight_paths, dtype, &device)
                .map_err(|e| FileLoaderError::ExtractionFailed(format!("load weights: {e}")))?
        };

        let model = moondream::Model::new(&moondream::Config::v2(), vb)
            .map_err(|e| FileLoaderError::ExtractionFailed(format!("model init: {e}")))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| FileLoaderError::ExtractionFailed(format!("tokenizer: {e}")))?;

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            tokenizer: Arc::new(tokenizer),
            device: Arc::new(device),
        })
    }

    fn load_weights(repo: &hf_hub::api::sync::ApiRepo) -> Result<Vec<PathBuf>, FileLoaderError> {
        if let Ok(path) = repo.get("model.safetensors") {
            tracing::info!("Loading single-shard model.safetensors");
            return Ok(vec![path]);
        }

        tracing::info!("model.safetensors not found, trying sharded layout via index.json");

        let index_path = repo.get("model.safetensors.index.json").map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("model.safetensors.index.json: {e}"))
        })?;

        let index_str = std::fs::read_to_string(&index_path)
            .map_err(|e| FileLoaderError::ExtractionFailed(format!("read index.json: {e}")))?;

        let shard_names = parse_shard_names(&index_str)?;

        tracing::info!(shard_count = shard_names.len(), "Downloading weight shards");

        let mut paths = Vec::with_capacity(shard_names.len());
        for name in &shard_names {
            let path = repo
                .get(name)
                .map_err(|e| FileLoaderError::ExtractionFailed(format!("shard {name}: {e}")))?;
            paths.push(path);
        }

        Ok(paths)
    }

    fn preprocess_image(png_bytes: &[u8], page_index: usize) -> Result<Tensor, FileLoaderError> {
        let img = image::load_from_memory(png_bytes).map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("image decode page {page_index}: {e}"))
        })?;

        let resized = img.resize_exact(
            MOONDREAM_IMAGE_SIZE,
            MOONDREAM_IMAGE_SIZE,
            FilterType::Triangle,
        );

        let rgb = resized.to_rgb8();
        let pixel_data: Vec<f32> = rgb
            .into_raw()
            .into_iter()
            .map(|p| (p as f32 / 255.0 - 0.5) / 0.5)
            .collect();

        Tensor::from_vec(
            pixel_data,
            (
                MOONDREAM_IMAGE_SIZE as usize,
                MOONDREAM_IMAGE_SIZE as usize,
                3,
            ),
            &Device::Cpu,
        )
        .and_then(|t| t.permute((2, 0, 1)))
        .and_then(|t| t.unsqueeze(0))
        .map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("image tensor page {page_index}: {e}"))
        })
    }

    async fn infer_page_markdown(
        &self,
        png_bytes: &[u8],
        page_index: usize,
    ) -> Result<String, FileLoaderError> {
        let model = Arc::clone(&self.model);
        let tokenizer = Arc::clone(&self.tokenizer);
        let device = Arc::clone(&self.device);
        let png_bytes = png_bytes.to_vec();

        tokio::task::spawn_blocking(move || {
            run_page_inference(&model, &tokenizer, &device, &png_bytes, page_index)
        })
        .await
        .map_err(|e| FileLoaderError::ExtractionFailed(format!("task join error: {e}")))?
    }
}

fn run_page_inference(
    model: &Mutex<moondream::Model>,
    tokenizer: &Tokenizer,
    device: &Device,
    png_bytes: &[u8],
    page_index: usize,
) -> Result<String, FileLoaderError> {
    let dtype = if device.is_cpu() {
        DType::F32
    } else {
        DType::F16
    };

    let img_tensor = LocalVlmPdfAdapter::preprocess_image(png_bytes, page_index)?
        .to_device(device)
        .map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("Failed to move image to GPU device: {e}"))
        })?
        .to_dtype(dtype)
        .map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("Failed to cast image to {dtype:?}: {e}"))
        })?;

    let encoding = tokenizer
        .encode(format!("\n\n{OCR_PROMPT}").as_str(), true)
        .map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("tokenize page {page_index}: {e}"))
        })?;

    let eos_token = tokenizer.token_to_id("<|endoftext|>").unwrap_or(u32::MAX);

    let prompt_ids: Vec<u32> = encoding.get_ids().to_vec();
    let bos_token = Tensor::new(&[eos_token], device)
        .and_then(|t| t.unsqueeze(0))
        .map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("bos tensor page {page_index}: {e}"))
        })?;
    let prompt_tensor = Tensor::new(prompt_ids.as_slice(), device)
        .and_then(|t| t.unsqueeze(0))
        .map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("prompt tensor page {page_index}: {e}"))
        })?;

    let mut model_guard = model.blocking_lock();

    model_guard.text_model().clear_kv_cache();

    let img_embeds = model_guard
        .vision_encoder()
        .forward(&img_tensor)
        .map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("vision encode page {page_index}: {e}"))
        })?;

    let logits = model_guard
        .text_model()
        .forward_with_img(&bos_token, &prompt_tensor, &img_embeds)
        .map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("forward_with_img page {page_index}: {e}"))
        })?;

    let mut next_token = logits
        .argmax(candle_core::D::Minus1)
        .and_then(|t| t.flatten_all())
        .and_then(|t| t.to_vec1::<u32>())
        .map(|v| v[0])
        .map_err(|e| FileLoaderError::ExtractionFailed(format!("argmax page {page_index}: {e}")))?;

    let mut generated: Vec<u32> = Vec::with_capacity(MOONDREAM_MAX_TOKENS);

    for _ in 0..MOONDREAM_MAX_TOKENS {
        if next_token == eos_token {
            break;
        }
        generated.push(next_token);

        let tail_start = generated.len().saturating_sub(4);
        if let Ok(tail_text) = tokenizer.decode(&generated[tail_start..], true) {
            if tail_text.contains("<END>") {
                tracing::info!(
                    "Moondream finished slide {} early at token {}",
                    page_index,
                    generated.len()
                );
                break;
            }
        }

        let next_tensor = Tensor::new(&[next_token], device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| {
                FileLoaderError::ExtractionFailed(format!(
                    "next token tensor page {page_index}: {e}"
                ))
            })?;

        next_token = model_guard
            .text_model()
            .forward(&next_tensor)
            .and_then(|l| l.argmax(candle_core::D::Minus1))
            .and_then(|t| t.flatten_all())
            .and_then(|t| t.to_vec1::<u32>())
            .map(|v| v[0])
            .map_err(|e| {
                FileLoaderError::ExtractionFailed(format!("forward page {page_index}: {e}"))
            })?;
    }

    let final_text = tokenizer
        .decode(&generated, true)
        .map_err(|e| FileLoaderError::ExtractionFailed(format!("decode page {page_index}: {e}")))?;

    Ok(final_text.replace("<END>", "").trim().to_string())
}

#[async_trait]
impl FileLoader for LocalVlmPdfAdapter {
    #[tracing::instrument(
        skip(self, data),
        fields(
            document_id = %document.id.as_uuid(),
            filename = %document.filename
        )
    )]
    async fn extract_text(
        &self,
        data: &[u8],
        document: &Document,
    ) -> Result<String, FileLoaderError> {
        if document.content_type != ContentType::Pdf {
            return Err(FileLoaderError::UnsupportedContentType(
                document.content_type.as_mime().to_string(),
            ));
        }

        let data_owned = data.to_vec();
        let filename = document.filename.clone();

        let png_buffers = tokio::time::timeout(
            EXTRACTION_TIMEOUT,
            tokio::task::spawn_blocking(move || {
                std::panic::catch_unwind(|| rasterize_pages(&data_owned)).unwrap_or_else(|_| {
                    Err(FileLoaderError::ExtractionFailed(
                        "OOM or panic during PDF rasterization".to_string(),
                    ))
                })
            }),
        )
        .await
        .map_err(|_| FileLoaderError::ExtractionFailed("PDF rasterization timed out".to_string()))?
        .map_err(|e| FileLoaderError::ExtractionFailed(format!("task join error: {e}")))??;

        if png_buffers.is_empty() {
            return Err(FileLoaderError::NoTextFound(filename));
        }

        tracing::info!(
            page_count = png_buffers.len(),
            "PDF rasterization complete, starting VLM inference"
        );

        let mut page_texts: Vec<String> = Vec::with_capacity(png_buffers.len());

        for (index, png_bytes) in png_buffers.iter().enumerate() {
            tracing::info!("Page: {index} infer to markdown");

            let page_text = self.infer_page_markdown(png_bytes, index).await?;
            if !page_text.trim().is_empty() {
                page_texts.push(sanitize_extracted_text(&page_text));
            }
        }

        if page_texts.is_empty() {
            return Err(FileLoaderError::NoTextFound(filename));
        }

        Ok(page_texts.join("\n\n"))
    }
}
