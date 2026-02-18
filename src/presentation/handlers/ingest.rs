use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;

use crate::application::ports::{FileLoader, LlmClient, TextSplitter, VectorStore};
use crate::application::services::IngestionMessage;
use crate::domain::{ContentType, DocumentId, Job, StoragePath};
use crate::presentation::state::AppState;

// TODO: Move those fields to the another file
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
    let mut field = match multipart.next_field().await {
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
    let content_type_str = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();

    tracing::debug!(filename = %filename, content_type = %content_type_str, "Processing file upload");

    let content_type = match ContentType::from_mime(&content_type_str) {
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

    let doc_id = DocumentId::new();
    let storage_path = StoragePath::new(&doc_id, &filename);

    // Stream multipart chunks directly to staging store without buffering
    let byte_stream: futures::stream::BoxStream<'_, Result<bytes::Bytes, std::io::Error>> =
        Box::pin(async_stream::stream! {
            loop {
                match field.chunk().await {
                    Ok(Some(bytes)) => yield Ok(bytes),
                    Ok(None) => break,
                    Err(e) => {
                        yield Err(std::io::Error::other(e.to_string()));
                        break;
                    }
                }
            }
        });

    let size_bytes = match state
        .staging_store
        .store(&storage_path, byte_stream, None)
        .await
    {
        Ok(size) => size,
        Err(e) => {
            tracing::error!(error = %e, "Failed to stage uploaded file");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Upload staging failed: {}", e),
                }),
            )
                .into_response();
        }
    };

    tracing::debug!(bytes = size_bytes, "File staged to storage");

    let document = crate::domain::Document {
        id: doc_id,
        filename: filename.clone(),
        content_type,
        size_bytes,
    };
    let job = Job::new(Some(doc_id), "document_ingestion".to_string());
    let job_id = job.id;

    if let Err(e) = state.job_repository.create(&job).await {
        tracing::error!(error = %e, "Failed to create job record");
        let _ = state.staging_store.delete(&storage_path).await;
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
        storage_path: storage_path.clone(),
        delete_after_processing: true,
    };

    if let Err(e) = state.ingestion_sender.send(msg).await {
        tracing::error!(error = %e, "Failed to enqueue ingestion job");
        let _ = state.staging_store.delete(&storage_path).await;
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
