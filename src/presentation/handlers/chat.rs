use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use futures::stream::StreamExt;
use serde::Serialize;
use std::convert::Infallible;
use std::time::Duration;

use crate::application::ports::{FileLoader, LlmClient, TextSplitter, VectorStore};
use crate::domain::{Message, MessageRole};
use crate::infrastructure::observability::sanitize_prompt;
use crate::presentation::state::AppState;

use super::openai_types::{ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse};

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
    fields(model = %request.model, streaming = ?request.stream)
)]
pub async fn chat_completions_handler<F, L, V, T>(
    State(state): State<AppState<F, L, V, T>>,
    Json(request): Json<ChatCompletionRequest>,
) -> impl IntoResponse
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
    T: TextSplitter + 'static + ?Sized,
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

    if request.stream == Some(true) {
        match state
            .retrieval_service
            .query_stream(&user_message, None)
            .await
        {
            Ok(streaming_response) => {
                let chunk_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
                let model = request.model.clone();
                let keep_alive_seconds = state.settings.llm.sse_keep_alive_seconds;
                let conversation_repo = state.conversation_repository.clone();
                let conversation_id = streaming_response.conversation_id;

                let sse_stream = async_stream::stream! {
                    let start_chunk = ChatCompletionChunk::new_start(&chunk_id, &model);
                    let start_json = serde_json::to_string(&start_chunk).unwrap_or_default();
                    yield Ok::<_, Infallible>(Event::default().data(start_json));

                    let mut accumulated_text = String::new();
                    let mut token_stream = streaming_response.token_stream;

                    loop {
                        tokio::select! {
                            token_result = token_stream.next() => {
                                match token_result {
                                    Some(Ok(token)) => {
                                        accumulated_text.push_str(&token);
                                        let content_chunk = ChatCompletionChunk::new_content(&chunk_id, &model, &token);
                                        let content_json = serde_json::to_string(&content_chunk).unwrap_or_default();
                                        yield Ok(Event::default().data(content_json));
                                    }
                                    Some(Err(e)) => {
                                        tracing::error!(error = %e, "Stream token error");
                                        if let Some(conv_id) = conversation_id {
                                            let partial_message = Message::new(
                                                conv_id,
                                                MessageRole::Assistant,
                                                format!("{} [TRUNCATED DUE TO ERROR]", accumulated_text)
                                            );
                                            let _ = conversation_repo.append_message(&partial_message).await;
                                        }
                                        break;
                                    }
                                    None => {
                                        if let Some(conv_id) = conversation_id {
                                            let user_msg = Message::new(conv_id, MessageRole::User, user_message.clone());
                                            let _ = conversation_repo.append_message(&user_msg).await;

                                            let assistant_msg = Message::new(conv_id, MessageRole::Assistant, accumulated_text.clone());
                                            let _ = conversation_repo.append_message(&assistant_msg).await;
                                        }

                                        let done_chunk = ChatCompletionChunk::new_done(&chunk_id, &model);
                                        let done_json = serde_json::to_string(&done_chunk).unwrap_or_default();
                                        yield Ok(Event::default().data(done_json));
                                        yield Ok(Event::default().data("[DONE]"));
                                        break;
                                    }
                                }
                            }
                            _ = tokio::time::sleep(Duration::from_secs(keep_alive_seconds)) => {
                                yield Ok(Event::default().comment("keep-alive"));
                            }
                        }
                    }
                };

                Sse::new(sse_stream)
                    .keep_alive(
                        axum::response::sse::KeepAlive::new()
                            .interval(Duration::from_secs(keep_alive_seconds))
                            .text("keep-alive"),
                    )
                    .into_response()
            }
            Err(e) => {
                tracing::error!(error = %e, "Streaming chat completion failed");
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
    } else {
        match state.retrieval_service.query(&user_message, None).await {
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
}
