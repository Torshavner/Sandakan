use sandakan::application::ports::{FileLoader, FileLoaderError};
use sandakan::domain::{ContentType, Document};
use sandakan::infrastructure::text_processing::PdfAdapter;

#[tokio::test]
async fn given_valid_pdf_bytes_when_extracting_then_returns_text() {
    let adapter = PdfAdapter::new();
    let pdf_bytes = include_bytes!("../fixtures/sample.pdf");
    let document = Document::new(
        "sample.pdf".to_string(),
        ContentType::Pdf,
        pdf_bytes.len() as u64,
    );

    let result = adapter.extract_text(pdf_bytes, &document).await;

    assert!(result.is_ok());
    let text = result.unwrap();
    assert!(!text.is_empty());
}

#[tokio::test]
async fn given_corrupt_bytes_when_extracting_pdf_then_returns_extraction_failed() {
    let adapter = PdfAdapter::new();
    let garbage = b"not a pdf at all";
    let document = Document::new(
        "corrupt.pdf".to_string(),
        ContentType::Pdf,
        garbage.len() as u64,
    );

    let result = adapter.extract_text(garbage, &document).await;

    assert!(matches!(result, Err(FileLoaderError::ExtractionFailed(_))));
}

#[tokio::test]
async fn given_empty_pdf_when_extracting_then_returns_no_text_found() {
    let adapter = PdfAdapter::new();
    let pdf_bytes = include_bytes!("../fixtures/empty.pdf");
    let document = Document::new(
        "empty.pdf".to_string(),
        ContentType::Pdf,
        pdf_bytes.len() as u64,
    );

    let result = adapter.extract_text(pdf_bytes, &document).await;

    assert!(matches!(result, Err(FileLoaderError::NoTextFound(_))));
}

#[tokio::test]
async fn given_non_pdf_content_type_when_extracting_then_returns_unsupported() {
    let adapter = PdfAdapter::new();
    let data = b"some data";
    let document = Document::new(
        "audio.mp3".to_string(),
        ContentType::Audio,
        data.len() as u64,
    );

    let result = adapter.extract_text(data, &document).await;

    assert!(matches!(
        result,
        Err(FileLoaderError::UnsupportedContentType(_))
    ));
}
