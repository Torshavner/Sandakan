use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::ports::{FileLoader, LlmClient, VectorStore};
use crate::domain::ConversationId;
use crate::infrastructure::observability::{CorrelationId, sanitize_prompt};
use crate::presentation::state::AppState;

#[derive(Deserialize)]
pub struct QueryRequest {
    pub question: String,
    pub conversation_id: Option<String>,
}

#[derive(Serialize)]
pub struct QueryResponse {
    pub answer: String,
    pub sources: Vec<SourceChunk>,
}

#[derive(Serialize)]
pub struct SourceChunk {
    pub text: String,
    pub page: Option<u32>,
    pub score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Source URL, with an appended `?t=Xs` / `&t=Xs` timestamp when the chunk is from
    /// a media file, enabling clickable deep-link citations (e.g. `youtube.com/watch?v=XYZ&t=1045s`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Start time of the chunk within the media file in seconds.
    /// Present only for audio/video sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<f32>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[tracing::instrument(skip(state, correlation_id, request))]
pub async fn query_handler<F, L, V>(
    State(state): State<AppState<F, L, V>>,
    Extension(correlation_id): Extension<CorrelationId>,
    Json(request): Json<QueryRequest>,
) -> impl IntoResponse
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
{
    tracing::debug!(question = %sanitize_prompt(&request.question), "Processing query");

    let conversation_id = request
        .conversation_id
        .and_then(|id| Uuid::parse_str(&id).ok())
        .map(ConversationId::from_uuid);

    match state
        .retrieval_service
        .query(&request.question, conversation_id, Some(correlation_id.0))
        .await
    {
        Ok(response) => {
            let citation_metadata_size = response
                .sources
                .iter()
                .filter(|s| s.start_time.is_some())
                .count();
            tracing::info!(
                sources_count = response.sources.len(),
                citation_metadata_size,
                "Query successful"
            );
            let sources = response
                .sources
                .into_iter()
                .map(|s| {
                    let timestamped = s.timestamped_url();
                    let start_time = s.start_time;
                    SourceChunk {
                        text: s.text,
                        page: s.page,
                        score: s.score,
                        title: s.title,
                        source_url: timestamped,
                        content_type: s.content_type,
                        start_time,
                    }
                })
                .collect();

            (
                StatusCode::OK,
                Json(QueryResponse {
                    answer: response.answer,
                    sources,
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Query failed: {}", e),
                }),
            )
                .into_response()
        }
    }
}
