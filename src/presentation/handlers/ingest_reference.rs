use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::application::ports::{
    FileLoader, LlmClient, StagingStoreError, TextSplitter, VectorStore,
};
use crate::application::services::IngestionMessage;
use crate::domain::{ContentType, Document, DocumentId, Job, StoragePath};
use crate::presentation::handlers::ingest::{ErrorResponse, IngestResponse};
use crate::presentation::state::AppState;

#[derive(Deserialize)]
pub struct IngestReferenceRequest {
    pub storage_path: String,
    pub filename: String,
    pub content_type: String,
}

pub async fn ingest_reference_handler<F, L, V, T>(
    State(state): State<AppState<F, L, V, T>>,
    Json(body): Json<IngestReferenceRequest>,
) -> impl IntoResponse
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
    T: TextSplitter + 'static + ?Sized,
{
    let content_type = match ContentType::from_mime(&body.content_type) {
        Some(ct) => ct,
        None => {
            tracing::warn!(content_type = %body.content_type, "Unsupported content type in reference request");
            return (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(ErrorResponse {
                    error: format!("Unsupported content type: {}", body.content_type),
                }),
            )
                .into_response();
        }
    };

    let storage_path = StoragePath::from_raw(&body.storage_path);

    let size_bytes = match state.staging_store.head(&storage_path).await {
        Ok(size) => size,
        Err(StagingStoreError::NotFound(_)) => {
            tracing::warn!(path = %body.storage_path, "Referenced file not found in storage");
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("File not found in storage: {}", body.storage_path),
                }),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to stat referenced file");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Storage error: {}", e),
                }),
            )
                .into_response();
        }
    };

    let doc_id = DocumentId::new();
    let document = Document {
        id: doc_id,
        filename: body.filename.clone(),
        content_type,
        size_bytes,
    };
    let job = Job::new(Some(doc_id), "document_ingestion".to_string());
    let job_id = job.id;

    if let Err(e) = state.job_repository.create(&job).await {
        tracing::error!(error = %e, "Failed to create job record for reference ingestion");
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
        storage_path,
        delete_after_processing: false,
    };

    if let Err(e) = state.ingestion_sender.send(msg).await {
        tracing::error!(error = %e, "Failed to enqueue reference ingestion job");
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
        filename = %body.filename,
        storage_path = %body.storage_path,
        "Reference ingestion job enqueued"
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
