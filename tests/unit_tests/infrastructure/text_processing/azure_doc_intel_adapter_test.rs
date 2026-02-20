use sandakan::application::ports::{FileLoader, FileLoaderError};
use sandakan::domain::{ContentType, Document};
use sandakan::infrastructure::text_processing::{AnalyzeResponse, AzureDocIntelAdapter};

fn make_document(filename: &str, content_type: ContentType) -> Document {
    Document::new(filename.to_string(), content_type, 0)
}

#[tokio::test]
async fn given_non_pdf_content_type_when_extracting_then_returns_unsupported() {
    let adapter = AzureDocIntelAdapter::new("https://example.cognitiveservices.azure.com", "key");
    let document = make_document("audio.mp3", ContentType::Audio);

    let result = adapter.extract_text(b"data", &document).await;

    assert!(matches!(
        result,
        Err(FileLoaderError::UnsupportedContentType(_))
    ));
}

#[tokio::test]
async fn given_succeeded_status_when_parsing_response_then_returns_content() {
    let json = r#"{"status":"succeeded","analyzeResult":{"content":"Title and body"}}"#;
    let parsed: AnalyzeResponse = serde_json::from_str(json).unwrap();

    assert_eq!(parsed.status, "succeeded");
    assert_eq!(parsed.analyze_result.unwrap().content, "Title and body");
}

#[tokio::test]
async fn given_failed_status_when_parsing_response_then_status_is_failed() {
    let json = r#"{"status":"failed"}"#;
    let parsed: AnalyzeResponse = serde_json::from_str(json).unwrap();

    assert_eq!(parsed.status, "failed");
    assert!(parsed.analyze_result.is_none());
}

#[tokio::test]
async fn given_running_status_when_parsing_response_then_no_result() {
    let json = r#"{"status":"running"}"#;
    let parsed: AnalyzeResponse = serde_json::from_str(json).unwrap();

    assert_eq!(parsed.status, "running");
    assert!(parsed.analyze_result.is_none());
}
