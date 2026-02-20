use std::convert::Infallible;
use std::path::PathBuf;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response};
use sandakan::application::ports::{FileLoader, FileLoaderError};
use sandakan::domain::{ContentType, Document};
use sandakan::infrastructure::text_processing::LmStudioVlmPdfAdapter;
use tokio::net::TcpListener;

fn sample_pdf_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/unit_tests/infrastructure/fixtures/sample.pdf")
}

async fn start_mock_lm_studio_server(
    status: u16,
    response_body: &'static str,
) -> (String, tokio::sync::oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://127.0.0.1:{}", addr.port());

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        let route = axum::routing::post(move |_req: Request<Body>| async move {
            Ok::<_, Infallible>(
                Response::builder()
                    .status(status)
                    .header("Content-Type", "application/json")
                    .body(Body::from(response_body))
                    .unwrap(),
            )
        });

        let app = Router::new().route("/v1/chat/completions", route);

        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });

    (base_url, shutdown_tx)
}

#[tokio::test]
async fn given_non_pdf_content_type_when_extracting_text_then_returns_unsupported_content_type() {
    let adapter = LmStudioVlmPdfAdapter::new("http://localhost:1234", "test-model", "test-key");
    let document = Document::new("file.txt".to_string(), ContentType::Text, 10);

    let result = adapter.extract_text(b"hello", &document).await;

    assert!(
        matches!(result, Err(FileLoaderError::UnsupportedContentType(_))),
        "expected UnsupportedContentType but got: {:?}",
        result
    );
}

#[tokio::test]
async fn given_valid_pdf_when_lm_studio_returns_content_then_returns_extracted_text() {
    let response_body = r#"{
        "choices": [{"message": {"content": "Invoice total: $1,234.56"}}]
    }"#;
    let (base_url, shutdown_tx) = start_mock_lm_studio_server(200, response_body).await;

    let adapter = LmStudioVlmPdfAdapter::new(&base_url, "test-model", "test-key");
    let data = std::fs::read(sample_pdf_path()).expect("sample.pdf fixture missing");
    let document = Document::new(
        "invoice.pdf".to_string(),
        ContentType::Pdf,
        data.len() as u64,
    );

    let result = adapter.extract_text(&data, &document).await;

    assert!(result.is_ok(), "expected Ok but got: {:?}", result);
    assert!(
        result.unwrap().contains("Invoice"),
        "expected extracted text to contain page content"
    );

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn given_valid_pdf_when_lm_studio_returns_error_status_then_returns_extraction_failed() {
    let (base_url, shutdown_tx) =
        start_mock_lm_studio_server(500, r#"{"error": "model overloaded"}"#).await;

    let adapter = LmStudioVlmPdfAdapter::new(&base_url, "test-model", "test-key");
    let data = std::fs::read(sample_pdf_path()).expect("sample.pdf fixture missing");
    let document = Document::new(
        "report.pdf".to_string(),
        ContentType::Pdf,
        data.len() as u64,
    );

    let result = adapter.extract_text(&data, &document).await;

    assert!(
        matches!(result, Err(FileLoaderError::ExtractionFailed(_))),
        "expected ExtractionFailed but got: {:?}",
        result
    );

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn given_valid_pdf_when_lm_studio_returns_empty_content_then_returns_no_text_found() {
    let response_body = r#"{"choices": [{"message": {"content": "   "}}]}"#;
    let (base_url, shutdown_tx) = start_mock_lm_studio_server(200, response_body).await;

    let adapter = LmStudioVlmPdfAdapter::new(&base_url, "test-model", "test-key");
    let data = std::fs::read(sample_pdf_path()).expect("sample.pdf fixture missing");
    let document = Document::new("blank.pdf".to_string(), ContentType::Pdf, data.len() as u64);

    let result = adapter.extract_text(&data, &document).await;

    assert!(
        matches!(result, Err(FileLoaderError::NoTextFound(_))),
        "expected NoTextFound but got: {:?}",
        result
    );

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn given_valid_pdf_when_lm_studio_returns_null_content_then_returns_no_text_found() {
    let response_body = r#"{"choices": [{"message": {"content": null}}]}"#;
    let (base_url, shutdown_tx) = start_mock_lm_studio_server(200, response_body).await;

    let adapter = LmStudioVlmPdfAdapter::new(&base_url, "test-model", "test-key");
    let data = std::fs::read(sample_pdf_path()).expect("sample.pdf fixture missing");
    let document = Document::new("blank.pdf".to_string(), ContentType::Pdf, data.len() as u64);

    let result = adapter.extract_text(&data, &document).await;

    assert!(
        matches!(result, Err(FileLoaderError::NoTextFound(_))),
        "expected NoTextFound but got: {:?}",
        result
    );

    let _ = shutdown_tx.send(());
}
