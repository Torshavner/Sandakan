use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;

use crate::application::ports::{FileLoader, LlmClient, TextSplitter, VectorStore};
use crate::application::services::IngestionMessage;
use crate::domain::{ContentType, Document, Job};
use crate::presentation::state::AppState;

#[derive(Serialize)]
pub struct IngestResponse {
    pub document_id: String,
    pub job_id: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[tracing::instrument(skip(state, multipart))]
pub async fn ingest_handler<F, L, V, T>(
    State(state): State<AppState<F, L, V, T>>,
    mut multipart: Multipart,
) -> impl IntoResponse
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
    T: TextSplitter + 'static + ?Sized,
{
    let field = match multipart.next_field().await {
        Ok(Some(f)) => f,
        Ok(None) => {
            tracing::warn!("Ingest request with no file");
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "No file uploaded".to_string(),
                }),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to read multipart");
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Failed to read multipart: {}", e),
                }),
            )
                .into_response();
        }
    };

    let filename = field.file_name().unwrap_or("unknown").to_string();
    let content_type_str = field.content_type().unwrap_or("application/octet-stream");

    tracing::debug!(filename = %filename, content_type = %content_type_str, "Processing file upload");

    let content_type = match ContentType::from_mime(content_type_str) {
        Some(ct) => ct,
        None => {
            tracing::warn!(content_type = %content_type_str, "Unsupported content type");
            return (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(ErrorResponse {
                    error: format!("Unsupported content type: {}", content_type_str),
                }),
            )
                .into_response();
        }
    };

    let data = match field.bytes().await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(error = %e, "Failed to read file bytes");
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Failed to read file: {}", e),
                }),
            )
                .into_response();
        }
    };

    tracing::debug!(bytes = data.len(), "File data received");

    let document = Document::new(filename.clone(), content_type, data.len() as u64);
    let doc_id = document.id;
    let job = Job::new(Some(doc_id), "document_ingestion".to_string());
    let job_id = job.id;

    if let Err(e) = state.job_repository.create(&job).await {
        tracing::error!(error = %e, "Failed to create job record");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create job: {}", e),
            }),
        )
            .into_response();
    }

    let msg = IngestionMessage {
        job_id,
        document,
        data: data.to_vec(),
    };

    if let Err(e) = state.ingestion_sender.send(msg).await {
        tracing::error!(error = %e, "Failed to enqueue ingestion job");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "Ingestion queue full or worker unavailable".to_string(),
            }),
        )
            .into_response();
    }

    tracing::info!(
        job_id = %job_id.as_uuid(),
        document_id = %doc_id.as_uuid(),
        filename = %filename,
        "Document ingestion job enqueued"
    );

    (
        StatusCode::ACCEPTED,
        Json(IngestResponse {
            document_id: doc_id.as_uuid().to_string(),
            job_id: job_id.as_uuid().to_string(),
            message: "Document ingestion started".to_string(),
        }),
    )
        .into_response()
}
