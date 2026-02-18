use sandakan::application::ports::{FileLoader, FileLoaderError};
use sandakan::domain::{ContentType, Document};
use sandakan::infrastructure::text_processing::PlainTextAdapter;

#[tokio::test]
async fn given_valid_utf8_bytes_when_extracting_then_returns_string() {
    let adapter = PlainTextAdapter;
    let text_bytes = b"Hello, this is plain text.";
    let document = Document::new(
        "readme.txt".to_string(),
        ContentType::Text,
        text_bytes.len() as u64,
    );

    let result = adapter.extract_text(text_bytes, &document).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Hello, this is plain text.");
}

#[tokio::test]
async fn given_invalid_utf8_bytes_when_extracting_then_returns_extraction_failed() {
    let adapter = PlainTextAdapter;
    let invalid_bytes: &[u8] = &[0xFF, 0xFE, 0xFD];
    let document = Document::new(
        "broken.txt".to_string(),
        ContentType::Text,
        invalid_bytes.len() as u64,
    );

    let result = adapter.extract_text(invalid_bytes, &document).await;

    assert!(matches!(result, Err(FileLoaderError::ExtractionFailed(_))));
}

#[tokio::test]
async fn given_non_text_content_type_when_extracting_then_returns_unsupported() {
    let adapter = PlainTextAdapter;
    let data = b"some data";
    let document = Document::new("file.pdf".to_string(), ContentType::Pdf, data.len() as u64);

    let result = adapter.extract_text(data, &document).await;

    assert!(matches!(
        result,
        Err(FileLoaderError::UnsupportedContentType(_))
    ));
}
