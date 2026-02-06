use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;

use crate::application::ports::{FileLoader, LlmClient, VectorStore};
use crate::infrastructure::observability::sanitize_prompt;
use crate::presentation::state::AppState;

use super::openai_types::{ChatCompletionRequest, ChatCompletionResponse};

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: ChatError,
}

#[derive(Serialize)]
pub struct ChatError {
    pub message: String,
    pub r#type: String,
}

#[tracing::instrument(
    skip(state, request),
    fields(model = %request.model)
)]
pub async fn chat_completions_handler<F, L, V>(
    State(state): State<AppState<F, L, V>>,
    Json(request): Json<ChatCompletionRequest>,
) -> impl IntoResponse
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

    tracing::debug!(prompt = %sanitize_prompt(&user_message), "Processing chat completion");

    if user_message.is_empty() {
        tracing::warn!("Chat completion request with empty user message");
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

    match state.retrieval_service.query(&user_message).await {
        Ok(response) => {
            tracing::info!("Chat completion successful");
            let chat_response = ChatCompletionResponse::new(request.model, response.answer);
            (StatusCode::OK, Json(chat_response)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Chat completion failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: ChatError {
                        message: format!("Query failed: {}", e),
                        r#type: "api_error".to_string(),
                    },
                }),
            )
                .into_response()
        }
    }
}
