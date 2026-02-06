use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;

use crate::application::ports::{FileLoader, LlmClient, VectorStore};
use crate::domain::ContentType;
use crate::presentation::state::AppState;

#[derive(Serialize)]
pub struct IngestResponse {
    pub document_id: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[tracing::instrument(skip(state, multipart))]
pub async fn ingest_handler<F, L, V>(
    State(state): State<AppState<F, L, V>>,
    mut multipart: Multipart,
) -> impl IntoResponse
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
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

    match state
        .ingestion_service
        .ingest(&data, filename.clone(), content_type)
        .await
    {
        Ok(doc_id) => {
            tracing::info!(
                document_id = %doc_id.as_uuid(),
                filename = %filename,
                "Document ingestion started"
            );
            (
                StatusCode::ACCEPTED,
                Json(IngestResponse {
                    document_id: doc_id.as_uuid().to_string(),
                    message: "Document ingestion started".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, filename = %filename, "Ingestion failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Ingestion failed: {}", e),
                }),
            )
                .into_response()
        }
    }
}
