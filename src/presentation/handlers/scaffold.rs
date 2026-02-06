use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures::stream;

use crate::application::ports::{FileLoader, LlmClient, VectorStore};
use crate::infrastructure::observability::sanitize_prompt;
use crate::presentation::state::AppState;

use super::chat::{ChatError, ErrorResponse};
use super::openai_types::{ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse};

#[tracing::instrument(skip(state, request), fields(model = %request.model, scaffold = true))]
pub async fn scaffold_chat_handler<F, L, V>(
    State(state): State<AppState<F, L, V>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Response
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
{
    let user_message = request
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    tracing::debug!(
        prompt = %sanitize_prompt(&user_message),
        request_body = ?request,
        "Scaffold mode: processing chat completion"
    );

    if user_message.is_empty() {
        tracing::warn!("Scaffold: empty user message");
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ChatError {
                    message: "No user message provided".to_string(),
                    r#type: "invalid_request_error".to_string(),
                },
            }),
        )
            .into_response();
    }

    let echo_response = format!("Echo: {}", user_message);

    if state.scaffold_config.mock_response_delay_ms > 0 {
        tokio::time::sleep(tokio::time::Duration::from_millis(
            state.scaffold_config.mock_response_delay_ms,
        ))
        .await;
    }

    if request.stream.unwrap_or(false) {
        tracing::info!("Scaffold: streaming response");
        create_streaming_response(&request.model, &echo_response)
    } else {
        tracing::info!("Scaffold: non-streaming response");
        let response = ChatCompletionResponse::new(request.model, echo_response);
        (StatusCode::OK, Json(response)).into_response()
    }
}

fn create_streaming_response(model: &str, content: &str) -> Response {
    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let model = model.to_string();
    let content = content.to_string();

    let words: Vec<String> = content.split_whitespace().map(String::from).collect();

    let start_chunk = ChatCompletionChunk::new_start(&id, &model);
    let start_data = format!("data: {}\n\n", serde_json::to_string(&start_chunk).unwrap());

    let id_clone = id.clone();
    let model_clone = model.clone();

    let content_chunks: Vec<String> = words
        .into_iter()
        .enumerate()
        .map(move |(i, word)| {
            let chunk_content = if i == 0 { word } else { format!(" {}", word) };
            let chunk = ChatCompletionChunk::new_content(&id_clone, &model_clone, &chunk_content);
            format!("data: {}\n\n", serde_json::to_string(&chunk).unwrap())
        })
        .collect();

    let done_chunk = ChatCompletionChunk::new_done(&id, &model);
    let done_data = format!("data: {}\n\n", serde_json::to_string(&done_chunk).unwrap());

    let mut all_chunks = vec![start_data];
    all_chunks.extend(content_chunks);
    all_chunks.push(done_data);
    all_chunks.push("data: [DONE]\n\n".to_string());

    let stream = stream::iter(
        all_chunks
            .into_iter()
            .map(Ok::<_, std::convert::Infallible>),
    );

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(Body::from_stream(stream))
        .unwrap()
}
