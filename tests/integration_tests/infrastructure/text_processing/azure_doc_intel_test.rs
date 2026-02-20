use std::convert::Infallible;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request, Response, StatusCode};
use sandakan::application::ports::{FileLoader, FileLoaderError};
use sandakan::domain::{ContentType, Document};
use sandakan::infrastructure::text_processing::AzureDocIntelAdapter;
use tokio::net::TcpListener;

/// Starts a minimal mock Azure Document Intelligence server on a random port.
/// Returns (base_url, shutdown_tx) — send `()` on shutdown_tx to stop the server.
async fn start_mock_azure_server(
    operation_response: &'static str,
) -> (String, tokio::sync::oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://127.0.0.1:{}", addr.port());
    let poll_url = format!("http://127.0.0.1:{}/operation/1", addr.port());
    let poll_url_clone = poll_url.clone();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        let analyze_route = {
            let poll_url = poll_url_clone.clone();
            axum::routing::post(move |_req: Request<Body>| {
                let poll = poll_url.clone();
                async move {
                    let mut resp = Response::new(Body::empty());
                    *resp.status_mut() = StatusCode::ACCEPTED;
                    resp.headers_mut()
                        .insert("Operation-Location", HeaderValue::from_str(&poll).unwrap());
                    Ok::<_, Infallible>(resp)
                }
            })
        };

        let poll_route = axum::routing::get(move || {
            let body = operation_response;
            async move {
                axum::response::Response::builder()
                    .status(200)
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .unwrap()
            }
        });

        let app = Router::new()
            .route(
                "/documentintelligence/documentModels/prebuilt-layout:analyze",
                analyze_route,
            )
            .route("/operation/1", poll_route);

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
async fn given_valid_pdf_bytes_when_azure_adapter_used_then_returns_markdown() {
    let response_body =
        r#"{"status":"succeeded","analyzeResult":{"content":"Report Some content here."}}"#;
    let (base_url, shutdown_tx) = start_mock_azure_server(response_body).await;

    let adapter = AzureDocIntelAdapter::new(&base_url, "test-api-key");
    let data = b"fake pdf bytes";
    let document = Document::new(
        "report.pdf".to_string(),
        ContentType::Pdf,
        data.len() as u64,
    );

    let result = adapter.extract_text(data, &document).await;

    assert!(result.is_ok(), "expected Ok but got: {:?}", result);
    let text = result.unwrap();
    assert!(
        text.contains("Report"),
        "expected markdown content, got: {text}"
    );

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn given_azure_returns_failed_status_when_polling_then_returns_extraction_failed() {
    let response_body = r#"{"status":"failed"}"#;
    let (base_url, shutdown_tx) = start_mock_azure_server(response_body).await;

    let adapter = AzureDocIntelAdapter::new(&base_url, "test-api-key");
    let data = b"corrupt pdf";
    let document = Document::new("bad.pdf".to_string(), ContentType::Pdf, data.len() as u64);

    let result = adapter.extract_text(data, &document).await;

    assert!(
        matches!(result, Err(FileLoaderError::ExtractionFailed(_))),
        "expected ExtractionFailed but got: {:?}",
        result
    );

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn given_azure_returns_empty_content_when_polling_then_returns_no_text_found() {
    let response_body = r#"{"status":"succeeded","analyzeResult":{"content":"   "}}"#;
    let (base_url, shutdown_tx) = start_mock_azure_server(response_body).await;

    let adapter = AzureDocIntelAdapter::new(&base_url, "test-api-key");
    let data = b"blank pdf";
    let document = Document::new("blank.pdf".to_string(), ContentType::Pdf, data.len() as u64);

    let result = adapter.extract_text(data, &document).await;

    assert!(
        matches!(result, Err(FileLoaderError::NoTextFound(_))),
        "expected NoTextFound but got: {:?}",
        result
    );

    let _ = shutdown_tx.send(());
}
