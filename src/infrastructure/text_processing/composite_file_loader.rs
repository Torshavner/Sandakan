use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::application::ports::{FileLoader, FileLoaderError};
use crate::domain::{ContentType, Document};

pub struct CompositeFileLoader {
    adapters: HashMap<ContentType, Arc<dyn FileLoader>>,
}

impl CompositeFileLoader {
    pub fn new(adapters: Vec<(ContentType, Arc<dyn FileLoader>)>) -> Self {
        Self {
            adapters: adapters.into_iter().collect(),
        }
    }
}

#[async_trait]
impl FileLoader for CompositeFileLoader {
    async fn extract_text(
        &self,
        data: &[u8],
        document: &Document,
    ) -> Result<String, FileLoaderError> {
        let adapter = self.adapters.get(&document.content_type).ok_or_else(|| {
            FileLoaderError::UnsupportedContentType(document.content_type.as_mime().to_string())
        })?;

        adapter.extract_text(data, document).await
    }
}
