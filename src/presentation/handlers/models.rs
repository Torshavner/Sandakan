use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use super::openai_types::ModelsResponse;

pub async fn models_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(ModelsResponse::with_rag_model()))
}
