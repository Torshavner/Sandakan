use async_trait::async_trait;

use crate::domain::Document;

#[async_trait]
pub trait FileLoader: Send + Sync {
    async fn extract_text(
        &self,
        data: &[u8],
        document: &Document,
    ) -> Result<String, FileLoaderError>;
}

#[derive(Debug, thiserror::Error)]
pub enum FileLoaderError {
    #[error("unsupported content type: {0}")]
    UnsupportedContentType(String),
    #[error("extraction failed: {0}")]
    ExtractionFailed(String),
}
