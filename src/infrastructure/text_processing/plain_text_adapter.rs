use async_trait::async_trait;

use crate::application::ports::{FileLoader, FileLoaderError};
use crate::domain::{ContentType, Document};

pub struct PlainTextAdapter;

#[async_trait]
impl FileLoader for PlainTextAdapter {
    async fn extract_text(
        &self,
        data: &[u8],
        document: &Document,
    ) -> Result<String, FileLoaderError> {
        if document.content_type != ContentType::Text {
            return Err(FileLoaderError::UnsupportedContentType(
                document.content_type.as_mime().to_string(),
            ));
        }

        String::from_utf8(data.to_vec())
            .map_err(|e| FileLoaderError::ExtractionFailed(e.to_string()))
    }
}
