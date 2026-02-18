use crate::application::ports::{FileLoader, FileLoaderError};
use crate::domain::Document;

pub struct MockFileLoader;

#[async_trait::async_trait]
impl FileLoader for MockFileLoader {
    async fn extract_text(&self, data: &[u8], _doc: &Document) -> Result<String, FileLoaderError> {
        String::from_utf8(data.to_vec())
            .map_err(|e| FileLoaderError::ExtractionFailed(e.to_string()))
    }
}