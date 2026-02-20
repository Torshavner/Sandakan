use axum::Router;
use axum::response::IntoResponse;
use axum::routing::post;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use sandakan::application::ports::TranscriptionEngine;
use sandakan::infrastructure::audio::AzureWhisperEngine;

async fn start_mock_azure_server(
    response_status: u16,
    response_body: &'static str,
) -> (String, oneshot::Sender<()>) {
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let app = Router::new().route(
        "/openai/deployments/my-deployment/audio/transcriptions",
        post(move || async move {
            let status = axum::http::StatusCode::from_u16(response_status).unwrap();
            (status, response_body).into_response()
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            })
            .await
            .ok();
    });

    (base_url, shutdown_tx)
}

#[tokio::test]
async fn given_valid_audio_bytes_when_azure_transcribes_then_returns_display_text() {
    let response_body = r#"{"text": "Hello from Azure Whisper"}"#;
    let (base_url, shutdown_tx) = start_mock_azure_server(200, response_body).await;

    let engine = AzureWhisperEngine::new(&base_url, "my-deployment", "test-key", "2024-02-01");
    let audio_data = b"fake audio bytes";

    let result = engine.transcribe(audio_data).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Hello from Azure Whisper");
    shutdown_tx.send(()).ok();
}

#[tokio::test]
async fn given_azure_api_returns_error_status_when_transcribing_then_returns_api_error() {
    let response_body = r#"{"error": {"code": "InvalidRequest", "message": "bad audio"}}"#;
    let (base_url, shutdown_tx) = start_mock_azure_server(400, response_body).await;

    let engine = AzureWhisperEngine::new(&base_url, "my-deployment", "test-key", "2024-02-01");
    let audio_data = b"bad audio";

    let result = engine.transcribe(audio_data).await;

    assert!(matches!(
        result,
        Err(sandakan::application::ports::TranscriptionError::ApiRequestFailed(_))
    ));
    shutdown_tx.send(()).ok();
}

#[tokio::test]
async fn given_azure_api_returns_empty_text_when_transcribing_then_returns_empty_string() {
    let response_body = r#"{"text": ""}"#;
    let (base_url, shutdown_tx) = start_mock_azure_server(200, response_body).await;

    let engine = AzureWhisperEngine::new(&base_url, "my-deployment", "test-key", "2024-02-01");
    let audio_data = b"silent audio";

    let result = engine.transcribe(audio_data).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
    shutdown_tx.send(()).ok();
}
