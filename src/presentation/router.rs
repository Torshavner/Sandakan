use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::application::ports::{FileLoader, LlmClient, VectorStore};
use crate::infrastructure::observability::{correlation_id_middleware, request_id_middleware};
use crate::presentation::handlers::{
    agent_chat_handler, chat_completions_handler, health_handler, ingest_handler,
    ingest_reference_handler, job_status_handler, models_handler, query_handler,
};
use crate::presentation::state::AppState;

pub fn create_router<F, L, V>(state: AppState<F, L, V>) -> Router
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
{
    let max_upload_bytes = state.settings.storage.max_upload_size_bytes as usize;

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    api_routes()
        .merge(openai_compat_routes::<F, L, V>())
        .layer(DefaultBodyLimit::max(max_upload_bytes))
        .layer(middleware::from_fn(correlation_id_middleware))
        .layer(middleware::from_fn(request_id_middleware))
        .layer(trace_layer)
        .layer(cors)
        .with_state(state)
}

/// Core API routes (health, ingestion, query, jobs, agent).
fn api_routes<F, L, V>() -> Router<AppState<F, L, V>>
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
{
    Router::new()
        .route("/openapi.json", get(serve_openapi_spec))
        .route("/health", get(health_handler))
        .route("/api/v1/ingest", post(ingest_handler::<F, L, V>))
        .route(
            "/api/v1/ingest-reference",
            post(ingest_reference_handler::<F, L, V>),
        )
        .route("/api/v1/query", post(query_handler::<F, L, V>))
        .route("/api/v1/jobs/{job_id}", get(job_status_handler::<F, L, V>))
        .route("/api/v1/agent/chat", post(agent_chat_handler::<F, L, V>))
}

/// OpenAI-compatible routes (canonical `/v1/` paths + `/api/` aliases for Open WebUI).
fn openai_compat_routes<F, L, V>() -> Router<AppState<F, L, V>>
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
{
    Router::new()
        .route("/v1/models", get(models_handler::<F, L, V>))
        .route(
            "/v1/chat/completions",
            post(chat_completions_handler::<F, L, V>),
        )
        // Open WebUI sends requests to /api/* instead of /v1/*
        .route("/api/models", get(models_handler::<F, L, V>))
        .route(
            "/api/chat/completions",
            post(chat_completions_handler::<F, L, V>),
        )
}

async fn serve_openapi_spec() -> impl IntoResponse {
    let spec = tokio::fs::read_to_string("openapi.json").await.unwrap();
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        spec,
    )
}
