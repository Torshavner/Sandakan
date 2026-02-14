use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use uuid::Uuid;

use crate::application::ports::{FileLoader, LlmClient, TextSplitter, VectorStore};
use crate::domain::JobId;
use crate::presentation::state::AppState;

#[derive(Serialize)]
pub struct JobStatusResponse {
    pub id: String,
    pub status: String,
    pub document_id: Option<String>,
    pub job_type: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[tracing::instrument(skip(state))]
pub async fn job_status_handler<F, L, V, T>(
    State(state): State<AppState<F, L, V, T>>,
    Path(job_id): Path<String>,
) -> impl IntoResponse
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
    T: TextSplitter + 'static + ?Sized,
{
    let uuid = match Uuid::parse_str(&job_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Invalid job ID: {}", job_id),
                }),
            )
                .into_response();
        }
    };

    match state.job_repository.get_by_id(JobId::from_uuid(uuid)).await {
        Ok(Some(job)) => {
            let response = JobStatusResponse {
                id: job.id.as_uuid().to_string(),
                status: job.status.as_str().to_string(),
                document_id: job.document_id.map(|id| id.as_uuid().to_string()),
                job_type: job.job_type,
                error_message: job.error_message,
                created_at: job.created_at.to_rfc3339(),
                updated_at: job.updated_at.to_rfc3339(),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Job not found: {}", job_id),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch job status");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to fetch job: {}", e),
                }),
            )
                .into_response()
        }
    }
}
