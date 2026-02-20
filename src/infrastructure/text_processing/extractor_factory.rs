use std::sync::Arc;

use crate::application::ports::FileLoader;
use crate::presentation::config::{ExtractorProvider, PdfExtractionSettings};

use super::azure_doc_intel_adapter::AzureDocIntelAdapter;
use super::lm_studio_vlm_pdf_adapter::LmStudioVlmPdfAdapter;
use super::local_vlm_pdf_adapter::LocalVlmPdfAdapter;

#[derive(Debug, thiserror::Error)]
pub enum ExtractorFactoryError {
    #[error("azure_endpoint is required for the Azure Document Intelligence provider")]
    MissingAzureEndpoint,
    #[error("azure_key is required for the Azure Document Intelligence provider")]
    MissingAzureKey,
    #[error("vlm_base_url is required for the LM Studio provider")]
    MissingVlmBaseUrl,
    #[error("vlm_model is required for the LM Studio provider")]
    MissingVlmModel,
    #[error("extractor initialization failed: {0}")]
    InitializationFailed(String),
}

pub struct ExtractorFactory;

impl ExtractorFactory {
    pub fn create(
        settings: &PdfExtractionSettings,
    ) -> Result<Arc<dyn FileLoader>, ExtractorFactoryError> {
        match settings.provider {
            ExtractorProvider::LocalVlm => {
                let model = settings
                    .vlm_model
                    .as_deref()
                    .unwrap_or("vikhyatk/moondream1");
                let revision = settings
                    .vlm_revision
                    .as_deref()
                    .or(Some("f6e9da68e8f1b78b8f3ee10905d56826db7a5802"));
                tracing::info!(model, "Loading local VLM PDF adapter");
                let adapter = LocalVlmPdfAdapter::new(model, revision)
                    .map_err(|e| ExtractorFactoryError::InitializationFailed(e.to_string()))?;
                Ok(Arc::new(adapter))
            }
            ExtractorProvider::LmStudio => {
                let base_url = settings
                    .vlm_base_url
                    .as_deref()
                    .ok_or(ExtractorFactoryError::MissingVlmBaseUrl)?;
                let model = settings
                    .vlm_model
                    .as_deref()
                    .ok_or(ExtractorFactoryError::MissingVlmModel)?;
                let api_key = settings.vlm_api_key.as_deref().unwrap_or("lm-studio");
                tracing::info!(model, base_url, "Loading LM Studio VLM PDF adapter");
                Ok(Arc::new(LmStudioVlmPdfAdapter::new(
                    base_url, model, api_key,
                )))
            }
            ExtractorProvider::Azure => {
                let endpoint = settings
                    .azure_endpoint
                    .as_deref()
                    .ok_or(ExtractorFactoryError::MissingAzureEndpoint)?;
                let key = settings
                    .azure_key
                    .as_deref()
                    .ok_or(ExtractorFactoryError::MissingAzureKey)?;
                tracing::info!("Loading Azure Document Intelligence PDF adapter");
                Ok(Arc::new(AzureDocIntelAdapter::new(endpoint, key)))
            }
        }
    }
}
