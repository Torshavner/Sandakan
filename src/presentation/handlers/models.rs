use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::application::ports::{FileLoader, LlmClient, VectorStore};
use crate::presentation::state::AppState;

use super::openai_types::ModelsResponse;

pub async fn models_handler<F, L, V>(State(state): State<AppState<F, L, V>>) -> impl IntoResponse
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
{
    let agent_enabled = state.agent_service.is_some();
    (
        StatusCode::OK,
        Json(ModelsResponse::with_models(agent_enabled)),
    )
}
