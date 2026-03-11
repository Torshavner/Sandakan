use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use futures::stream::StreamExt;
use serde::Serialize;
use std::convert::Infallible;
use std::time::Duration;

use crate::application::ports::{FileLoader, LlmClient, VectorStore};
use crate::application::services::AgentChatRequest;
use crate::domain::{ConversationId, Message, MessageRole};
use crate::infrastructure::observability::{CorrelationId, sanitize_prompt};
use crate::presentation::config::ChatMode;
use crate::presentation::state::AppState;

use super::openai_types::{ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse};

const AGENT_MODEL_ID: &str = "agent-pipeline";

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: ChatError,
}

#[derive(Serialize)]
pub struct ChatError {
    pub message: String,
    pub r#type: String,
}

/// Returns `true` when the request should be handled by `AgentService`.
///
/// Two triggers:
/// 1. The caller explicitly selected `"agent-pipeline"` as the model name.
/// 2. The operator set `agent.chat_mode = "agent"` in config (default-routes all traffic to agent).
fn should_use_agent(
    request: &ChatCompletionRequest,
    agent_enabled: bool,
    chat_mode: &ChatMode,
) -> bool {
    if !agent_enabled {
        return false;
    }
    request.model == AGENT_MODEL_ID || *chat_mode == ChatMode::Agent
}

#[tracing::instrument(
    skip(state, correlation_id, request),
    fields(model = %request.model, streaming = ?request.stream)
)]
pub async fn chat_completions_handler<F, L, V>(
    State(state): State<AppState<F, L, V>>,
    Extension(correlation_id): Extension<CorrelationId>,
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

    let agent_service = state.agent_service.clone();
    let use_agent = should_use_agent(
        &request,
        agent_service.is_some(),
        &state.settings.agent.chat_mode,
    );

    if use_agent {
        let service = match agent_service {
            Some(s) => s,
            None => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorResponse {
                        error: ChatError {
                            message: "Agent service is not enabled".to_string(),
                            r#type: "api_error".to_string(),
                        },
                    }),
                )
                    .into_response();
            }
        };

        let conversation_id = request.messages.iter().find_map(|m| {
            // Carry forward conversation_id if the client embeds it in a system message metadata.
            // Standard path: no existing conversation — AgentService creates one.
            let _ = m;
            None::<ConversationId>
        });

        let agent_request = AgentChatRequest {
            conversation_id,
            user_message: user_message.clone(),
            correlation_id: Some(correlation_id.0),
        };

        // Non-streaming: run the agent synchronously and return a single JSON response.
        if request.stream != Some(true) {
            match service.chat(agent_request).await {
                Ok(response) => {
                    let model = request.model.clone();
                    let mut full_text = String::new();
                    let mut token_stream = response.token_stream;
                    while let Some(result) = token_stream.next().await {
                        match result {
                            Ok(token) => full_text.push_str(&token),
                            Err(e) => {
                                tracing::error!(error = %e, "Agent token stream error (non-streaming)");
                                return (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(ErrorResponse {
                                        error: ChatError {
                                            message: format!("Agent failed: {}", e),
                                            r#type: "api_error".to_string(),
                                        },
                                    }),
                                )
                                    .into_response();
                            }
                        }
                    }
                    let chat_response = ChatCompletionResponse::new(model, full_text);
                    return (StatusCode::OK, Json(chat_response)).into_response();
                }
                Err(e) => {
                    tracing::error!(error = %e, "Agent chat failed");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: ChatError {
                                message: format!("Agent failed: {}", e),
                                r#type: "api_error".to_string(),
                            },
                        }),
                    )
                        .into_response();
                }
            }
        }

        // Streaming: spawn the agent as a background task so the SSE response
        // starts immediately. Progress events are forwarded as SSE comments so
        // Open WebUI stays responsive during the (potentially long) ReAct loop.
        let chunk_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
        let model = request.model.clone();
        let keep_alive_secs = state.settings.llm.sse_keep_alive_seconds;

        // Channel that carries the finished AgentChatResponse back to the SSE stream.
        let (result_tx, mut result_rx) =
            tokio::sync::oneshot::channel::<Result<crate::application::services::AgentChatResponse, crate::application::errors::AgentError>>();

        tokio::spawn(async move {
            let _ = result_tx.send(service.chat(agent_request).await);
        });

        let sse_stream = async_stream::stream! {
            let start_chunk = ChatCompletionChunk::new_start(&chunk_id, &model);
            let start_json = serde_json::to_string(&start_chunk).unwrap_or_default();
            yield Ok::<_, Infallible>(Event::default().data(start_json));

            // Wait for the agent to finish, emitting keep-alive comments so the
            // connection is not dropped by proxies or the browser.
            let agent_result = loop {
                tokio::select! {
                    res = &mut result_rx => {
                        break res.unwrap_or_else(|_| Err(crate::application::errors::AgentError::MaxIterationsExceeded(0)));
                    }
                    _ = tokio::time::sleep(Duration::from_secs(keep_alive_secs)) => {
                        yield Ok(Event::default().comment("keep-alive"));
                    }
                }
            };

            match agent_result {
                Err(e) => {
                    tracing::error!(error = %e, "Agent chat failed (streaming)");
                    // Emit an error token so the UI shows something meaningful.
                    let err_chunk = ChatCompletionChunk::new_content(&chunk_id, &model, &format!("[Agent error: {e}]"));
                    let err_json = serde_json::to_string(&err_chunk).unwrap_or_default();
                    yield Ok(Event::default().data(err_json));
                }
                Ok(response) => {
                    // Drain progress events for observability only.
                    let mut progress_rx = response.progress_rx;
                    while let Ok(event) = progress_rx.try_recv() {
                        tracing::debug!(event = ?event, "Agent progress event");
                    }

                    let mut token_stream = response.token_stream;
                    while let Some(result) = token_stream.next().await {
                        match result {
                            Ok(token) => {
                                let chunk = ChatCompletionChunk::new_content(&chunk_id, &model, &token);
                                let json = serde_json::to_string(&chunk).unwrap_or_default();
                                yield Ok(Event::default().data(json));
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "Agent token stream error");
                                break;
                            }
                        }
                    }
                }
            }

            let done_chunk = ChatCompletionChunk::new_done(&chunk_id, &model);
            let done_json = serde_json::to_string(&done_chunk).unwrap_or_default();
            yield Ok(Event::default().data(done_json));
            yield Ok(Event::default().data("[DONE]"));
        };

        Sse::new(sse_stream)
            .keep_alive(
                axum::response::sse::KeepAlive::new()
                    .interval(Duration::from_secs(keep_alive_secs))
                    .text("keep-alive"),
            )
            .into_response()
    } else if request.stream == Some(true) {
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
        match state
            .retrieval_service
            .query(&user_message, None, Some(correlation_id.0))
            .await
        {
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
