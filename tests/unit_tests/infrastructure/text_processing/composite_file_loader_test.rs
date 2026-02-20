use std::sync::Arc;

use sandakan::application::ports::{FileLoader, FileLoaderError};
use sandakan::domain::{ContentType, Document};
use sandakan::infrastructure::text_processing::{
    CompositeFileLoader, MockFileLoader, PlainTextAdapter,
};

#[tokio::test]
async fn given_pdf_document_when_loading_then_delegates_to_registered_adapter() {
    let pdf_adapter: Arc<dyn FileLoader> = Arc::new(MockFileLoader);
    let text_adapter: Arc<dyn FileLoader> = Arc::new(PlainTextAdapter);
    let loader = CompositeFileLoader::new(vec![
        (ContentType::Pdf, pdf_adapter),
        (ContentType::Text, text_adapter),
    ]);

    let data = b"mock pdf bytes";
    let document = Document::new(
        "sample.pdf".to_string(),
        ContentType::Pdf,
        data.len() as u64,
    );

    let result = loader.extract_text(data, &document).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn given_text_document_when_loading_then_delegates_to_text_adapter() {
    let pdf_adapter: Arc<dyn FileLoader> = Arc::new(MockFileLoader);
    let text_adapter: Arc<dyn FileLoader> = Arc::new(PlainTextAdapter);
    let loader = CompositeFileLoader::new(vec![
        (ContentType::Pdf, pdf_adapter),
        (ContentType::Text, text_adapter),
    ]);

    let text_bytes = b"Hello plain text";
    let document = Document::new(
        "readme.txt".to_string(),
        ContentType::Text,
        text_bytes.len() as u64,
    );

    let result = loader.extract_text(text_bytes, &document).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Hello plain text");
}

#[tokio::test]
async fn given_unregistered_content_type_when_loading_then_returns_unsupported() {
    let text_adapter: Arc<dyn FileLoader> = Arc::new(PlainTextAdapter);
    let loader = CompositeFileLoader::new(vec![(ContentType::Text, text_adapter)]);

    let data = b"fake audio";
    let document = Document::new(
        "lecture.mp3".to_string(),
        ContentType::Audio,
        data.len() as u64,
    );

    let result = loader.extract_text(data, &document).await;

    assert!(matches!(
        result,
        Err(FileLoaderError::UnsupportedContentType(_))
    ));
}
