use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use futures::StreamExt;
use serde::Deserialize;
use std::convert::Infallible;
use std::time::Duration;

use crate::application::ports::{FileLoader, LlmClient, VectorStore};
use crate::application::services::AgentChatRequest;
use crate::domain::ConversationId;
use crate::infrastructure::observability::CorrelationId;
use crate::presentation::state::AppState;

#[derive(Deserialize)]
pub struct AgentChatRequestBody {
    pub message: String,
    pub conversation_id: Option<String>,
}

#[tracing::instrument(
    skip(state, correlation_id, body),
    fields(message_len = body.message.len())
)]
pub async fn agent_chat_handler<F, L, V>(
    State(state): State<AppState<F, L, V>>,
    Extension(correlation_id): Extension<CorrelationId>,
    Json(body): Json<AgentChatRequestBody>,
) -> impl IntoResponse
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
{
    let service = match &state.agent_service {
        Some(s) => s.clone(),
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    if body.message.trim().is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let conversation_id = body
        .conversation_id
        .as_deref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .map(ConversationId::from_uuid);

    let request = AgentChatRequest {
        conversation_id,
        user_message: body.message,
        correlation_id: Some(correlation_id.0),
    };

    match service.chat(request).await {
        Ok(response) => {
            let keep_alive_secs = state.settings.llm.sse_keep_alive_seconds;
            let mut progress_rx = response.progress_rx;
            let mut token_stream = response.token_stream;

            let sse_stream = async_stream::stream! {
                // Drain progress events. The service drops the sender before returning,
                // so all events are already buffered in the channel.
                while let Ok(event) = progress_rx.try_recv() {
                    let json = serde_json::to_string(&event).unwrap_or_default();
                    yield Ok::<_, Infallible>(Event::default().data(json));
                }

                // Stream final answer tokens.
                loop {
                    tokio::select! {
                        maybe_token = token_stream.next() => {
                            match maybe_token {
                                Some(Ok(token)) => {
                                    let payload = serde_json::json!({
                                        "type": "token",
                                        "content": token
                                    });
                                    yield Ok(Event::default().data(payload.to_string()));
                                }
                                Some(Err(e)) => {
                                    tracing::error!(error = %e, "Agent token stream error");
                                    break;
                                }
                                None => break,
                            }
                        }
                        _ = tokio::time::sleep(Duration::from_secs(keep_alive_secs)) => {
                            yield Ok(Event::default().comment("keep-alive"));
                        }
                    }
                }

                yield Ok(Event::default().data(r#"{"type":"done"}"#));
            };

            Sse::new(sse_stream)
                .keep_alive(
                    axum::response::sse::KeepAlive::new()
                        .interval(Duration::from_secs(keep_alive_secs))
                        .text("keep-alive"),
                )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Agent chat failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
