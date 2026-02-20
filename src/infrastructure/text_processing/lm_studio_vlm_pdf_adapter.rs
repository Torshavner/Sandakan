use std::time::Duration;

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use reqwest::Client;
use serde::Deserialize;

use crate::application::ports::{FileLoader, FileLoaderError};
use crate::domain::{ContentType, Document};

use super::local_vlm_pdf_adapter::{EXTRACTION_TIMEOUT, OCR_PROMPT};
use super::pdf_rasterizer::rasterize_pages;
use super::text_sanitizer::sanitize_extracted_text;

pub struct LmStudioVlmPdfAdapter {
    client: Client,
    base_url: String,
    model: String,
    api_key: String,
}

impl LmStudioVlmPdfAdapter {
    pub const VLM_TIMEOUT: Duration = Duration::from_secs(300);
    pub fn new(base_url: &str, model: &str, api_key: &str) -> Self {
        let client = Client::builder()
            .timeout(Self::VLM_TIMEOUT)
            .build()
            .expect("reqwest client build never fails with valid TLS config");
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key: api_key.to_string(),
        }
    }

    async fn infer_page_markdown(
        &self,
        png_bytes: &[u8],
        page_index: usize,
    ) -> Result<String, FileLoaderError> {
        let b64 = general_purpose::STANDARD.encode(png_bytes);
        let data_uri = format!("data:image/png;base64,{b64}");

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "image_url",
                            "image_url": { "url": data_uri }
                        },
                        {
                            "type": "text",
                            "text": OCR_PROMPT
                        }
                    ]
                }
            ],
            "max_tokens": 2048,
            "temperature": 0.0,
            "stream": false
        });

        let url = format!("{}/v1/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                FileLoaderError::ExtractionFailed(format!(
                    "LM Studio request page {page_index}: {e}"
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(FileLoaderError::ExtractionFailed(format!(
                "LM Studio returned {status} page {page_index}: {text}"
            )));
        }

        let raw_bytes = response.bytes().await.map_err(|e| {
            FileLoaderError::ExtractionFailed(format!(
                "LM Studio network/read error page {page_index}: {e}"
            ))
        })?;

        let completion: ChatCompletion = serde_json::from_slice(&raw_bytes).map_err(|e| {
            let raw_text = String::from_utf8_lossy(&raw_bytes);
            tracing::error!(
                page_index,
                raw_response = %raw_text,
                "Failed to parse LM Studio JSON"
            );
            FileLoaderError::ExtractionFailed(format!(
                "LM Studio JSON parse error page {page_index}: {e}"
            ))
        })?;

        let content = completion
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();

        Ok(content)
    }
}

#[derive(Deserialize)]
struct ChatCompletion {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[async_trait]
impl FileLoader for LmStudioVlmPdfAdapter {
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
            "PDF rasterization complete, starting LM Studio VLM inference"
        );

        let mut page_texts: Vec<String> = Vec::with_capacity(png_buffers.len());

        for (index, png_bytes) in png_buffers.iter().enumerate() {
            let page_text = self.infer_page_markdown(png_bytes, index).await?;
            if !page_text.trim().is_empty() {
                tracing::debug!("Page: {index} sanitize to markdown: {page_text}");

                page_texts.push(sanitize_extracted_text(&page_text));
            }
        }

        if page_texts.is_empty() {
            return Err(FileLoaderError::NoTextFound(filename));
        }

        Ok(page_texts.join("\n\n"))
    }
}
