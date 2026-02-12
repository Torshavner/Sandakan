use std::io::Write;
use std::time::Duration;

use async_trait::async_trait;
use pdf_oxide::PdfDocument;

use crate::application::ports::{FileLoader, FileLoaderError};
use crate::domain::{ContentType, Document};

use super::text_sanitizer::sanitize_extracted_text;

const EXTRACTION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct PdfAdapter;

struct PageContent {
    #[allow(dead_code)]
    page_number: u32,
    text: String,
}

impl PdfAdapter {
    pub fn new() -> Self {
        Self
    }

    fn extract_pages(path: &std::path::Path) -> Result<Vec<PageContent>, FileLoaderError> {
        let mut doc = PdfDocument::open(path)
            .map_err(|e| FileLoaderError::ExtractionFailed(format!("failed to parse PDF: {e}")))?;

        let page_count = doc.page_count().map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("failed to read page count: {e}"))
        })?;

        let mut pages = Vec::with_capacity(page_count);

        for page_index in 0..page_count {
            let text = doc.extract_text(page_index).unwrap_or_default();

            if !text.trim().is_empty() {
                pages.push(PageContent {
                    page_number: (page_index + 1) as u32,
                    text,
                });
            }
        }

        Ok(pages)
    }
}

#[async_trait]
impl FileLoader for PdfAdapter {
    #[tracing::instrument(
        skip(self, data),
        fields(
            document_id = %document.id.as_uuid(),
            filename = %document.filename,
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

        let mut temp_file = tempfile::NamedTempFile::new().map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("failed to create temp file: {e}"))
        })?;

        temp_file.write_all(data).map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("failed to write temp file: {e}"))
        })?;

        let temp_path = temp_file.path().to_path_buf();
        let filename = document.filename.clone();

        let pages = tokio::time::timeout(
            EXTRACTION_TIMEOUT,
            tokio::task::spawn_blocking(move || Self::extract_pages(&temp_path)),
        )
        .await
        .map_err(|_| FileLoaderError::ExtractionFailed("PDF extraction timed out".to_string()))?
        .map_err(|e| FileLoaderError::ExtractionFailed(format!("task join error: {e}")))??;

        let page_count = pages.len();
        tracing::info!(page_count, "PDF text extraction complete");

        if pages.is_empty() {
            return Err(FileLoaderError::NoTextFound(filename));
        }

        let sanitized_pages: Vec<String> = pages
            .into_iter()
            .map(|p| sanitize_extracted_text(&p.text))
            .filter(|t| !t.is_empty())
            .collect();

        if sanitized_pages.is_empty() {
            return Err(FileLoaderError::NoTextFound(filename));
        }

        Ok(sanitized_pages.join("\n\n"))
    }
}
