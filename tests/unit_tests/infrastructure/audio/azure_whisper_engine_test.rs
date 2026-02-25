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
async fn given_valid_audio_bytes_when_azure_transcribes_then_returns_timed_segments() {
    let response_body = r#"{"segments": [{"id": 0, "start": 0.0, "end": 3.5, "text": "Hello from Azure Whisper"}]}"#;
    let (base_url, shutdown_tx) = start_mock_azure_server(200, response_body).await;

    let engine = AzureWhisperEngine::new(&base_url, "my-deployment", "test-key", "2024-02-01");
    let audio_data = b"fake audio bytes";

    let result = engine.transcribe(audio_data).await;

    assert!(result.is_ok());
    let segments = result.unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].text, "Hello from Azure Whisper");
    assert!((segments[0].start_time - 0.0).abs() < f32::EPSILON);
    assert!((segments[0].end_time - 3.5).abs() < 0.01);
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
async fn given_azure_api_returns_empty_segments_when_transcribing_then_returns_empty_vec() {
    let response_body = r#"{"segments": []}"#;
    let (base_url, shutdown_tx) = start_mock_azure_server(200, response_body).await;

    let engine = AzureWhisperEngine::new(&base_url, "my-deployment", "test-key", "2024-02-01");
    let audio_data = b"silent audio";

    let result = engine.transcribe(audio_data).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
    shutdown_tx.send(()).ok();
}

#[tokio::test]
async fn given_multiple_segments_when_azure_transcribes_then_all_segments_carry_accurate_timestamps()
 {
    let response_body = r#"{
        "segments": [
            {"id": 0, "start": 0.0,  "end": 5.0,  "text": "First segment."},
            {"id": 1, "start": 5.2,  "end": 10.0, "text": "Second segment."},
            {"id": 2, "start": 10.5, "end": 15.3, "text": "Third segment."}
        ]
    }"#;
    let (base_url, shutdown_tx) = start_mock_azure_server(200, response_body).await;

    let engine = AzureWhisperEngine::new(&base_url, "my-deployment", "test-key", "2024-02-01");

    let result = engine.transcribe(b"audio").await.unwrap();

    assert_eq!(result.len(), 3);
    assert!((result[0].start_time - 0.0).abs() < f32::EPSILON);
    assert!((result[1].start_time - 5.2).abs() < 0.01);
    assert!((result[2].start_time - 10.5).abs() < 0.01);
    shutdown_tx.send(()).ok();
}
