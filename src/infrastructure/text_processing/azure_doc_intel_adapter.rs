use std::time::Duration;

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use reqwest::Client;
use serde::Deserialize;

use crate::application::ports::{FileLoader, FileLoaderError};
use crate::domain::{ContentType, Document};

pub const POLL_TIMEOUT: Duration = Duration::from_secs(300);
pub const INITIAL_BACKOFF: Duration = Duration::from_secs(2);
pub const MAX_BACKOFF: Duration = Duration::from_secs(60);
pub const API_VERSION: &str = "2024-11-30";

pub struct AzureDocIntelAdapter {
    client: Client,
    endpoint: String,
    api_key: String,
}

impl AzureDocIntelAdapter {
    pub fn new(endpoint: &str, api_key: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client build never fails with valid TLS config");
        Self {
            client,
            endpoint: endpoint.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }

    async fn submit(&self, data: &[u8]) -> Result<String, FileLoaderError> {
        let b64 = general_purpose::STANDARD.encode(data);
        let body = serde_json::json!({ "base64Source": b64 });

        let url = format!(
            "{}/documentintelligence/documentModels/prebuilt-layout:analyze?api-version={}&outputContentFormat=markdown",
            self.endpoint, API_VERSION
        );

        let response = self
            .client
            .post(&url)
            .header("Ocp-Apim-Subscription-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| FileLoaderError::ExtractionFailed(format!("Azure submit failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(FileLoaderError::ExtractionFailed(format!(
                "Azure submit returned {status}: {text}"
            )));
        }

        let operation_url = response
            .headers()
            .get("Operation-Location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                FileLoaderError::ExtractionFailed(
                    "Azure response missing Operation-Location header".to_string(),
                )
            })?
            .to_string();

        Ok(operation_url)
    }

    async fn poll_until_complete(&self, operation_url: &str) -> Result<String, FileLoaderError> {
        let poll_future = async {
            let mut backoff = INITIAL_BACKOFF;

            loop {
                let response = self
                    .client
                    .get(operation_url)
                    .header("Ocp-Apim-Subscription-Key", &self.api_key)
                    .send()
                    .await
                    .map_err(|e| {
                        FileLoaderError::ExtractionFailed(format!("Azure poll request failed: {e}"))
                    })?;

                if response.status().as_u16() == 429 {
                    let retry_after = response
                        .headers()
                        .get("Retry-After")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(backoff.as_secs());
                    tokio::time::sleep(Duration::from_secs(retry_after)).await;
                    continue;
                }

                if !response.status().is_success() {
                    let status = response.status();
                    let text = response.text().await.unwrap_or_default();
                    return Err(FileLoaderError::ExtractionFailed(format!(
                        "Azure poll returned {status}: {text}"
                    )));
                }

                let result: AnalyzeResponse = response.json().await.map_err(|e| {
                    FileLoaderError::ExtractionFailed(format!("Azure response parse failed: {e}"))
                })?;

                match result.status.as_str() {
                    "succeeded" => {
                        let content = result.analyze_result.map(|r| r.content).unwrap_or_default();
                        return Ok(content);
                    }
                    "failed" => {
                        return Err(FileLoaderError::ExtractionFailed(
                            "Azure Document Intelligence analysis failed".to_string(),
                        ));
                    }
                    _ => {
                        tokio::time::sleep(backoff).await;
                        backoff = (backoff * 2).min(MAX_BACKOFF);
                    }
                }
            }
        };

        tokio::time::timeout(POLL_TIMEOUT, poll_future)
            .await
            .map_err(|_| {
                FileLoaderError::ExtractionFailed(
                    "Azure Document Intelligence polling timed out after 300s".to_string(),
                )
            })?
    }
}

#[async_trait]
impl FileLoader for AzureDocIntelAdapter {
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

        let operation_url = self.submit(data).await?;
        let markdown = self.poll_until_complete(&operation_url).await?;

        if markdown.trim().is_empty() {
            return Err(FileLoaderError::NoTextFound(document.filename.clone()));
        }

        Ok(markdown)
    }
}

#[derive(Deserialize)]
pub struct AnalyzeResponse {
    pub status: String,
    #[serde(rename = "analyzeResult")]
    pub analyze_result: Option<AnalyzeResult>,
}

#[derive(Deserialize)]
pub struct AnalyzeResult {
    pub content: String,
}
