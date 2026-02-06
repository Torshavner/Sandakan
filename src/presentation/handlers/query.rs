use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::application::ports::{FileLoader, LlmClient, VectorStore};
use crate::infrastructure::observability::sanitize_prompt;
use crate::presentation::state::AppState;

#[derive(Deserialize)]
pub struct QueryRequest {
    pub question: String,
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
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[tracing::instrument(skip(state, request))]
pub async fn query_handler<F, L, V>(
    State(state): State<AppState<F, L, V>>,
    Json(request): Json<QueryRequest>,
) -> impl IntoResponse
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
{
    tracing::debug!(question = %sanitize_prompt(&request.question), "Processing query");

    match state.retrieval_service.query(&request.question).await {
        Ok(response) => {
            tracing::info!(sources_count = response.sources.len(), "Query successful");
            let sources = response
                .sources
                .into_iter()
                .map(|s| SourceChunk {
                    text: s.text,
                    page: s.page,
                    score: s.score,
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
