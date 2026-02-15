use axum::Router;
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::application::ports::{FileLoader, LlmClient, TextSplitter, VectorStore};
use crate::infrastructure::observability::request_id_middleware;
use crate::presentation::handlers::{
    chat_completions_handler, health_handler, ingest_handler, job_status_handler, models_handler,
    query_handler,
};
use crate::presentation::state::AppState;

pub fn create_router<F, L, V, T>(state: AppState<F, L, V, T>) -> Router
where
    F: FileLoader + 'static,
    L: LlmClient + 'static,
    V: VectorStore + 'static,
    T: TextSplitter + 'static + ?Sized,
{
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    let mut router = Router::new()
        .route("/openapi.json", get(serve_openapi_spec))
        .route("/health", get(health_handler))
        .route("/api/v1/ingest", post(ingest_handler::<F, L, V, T>))
        .route("/api/v1/query", post(query_handler::<F, L, V, T>))
        .route(
            "/api/v1/jobs/{job_id}",
            get(job_status_handler::<F, L, V, T>),
        )
        .route("/v1/models", get(models_handler))
        .route("/api/models", get(models_handler));

    router = router
        .route(
            "/v1/chat/completions",
            post(chat_completions_handler::<F, L, V, T>),
        )
        .route(
            "/api/chat/completions",
            post(chat_completions_handler::<F, L, V, T>),
        );

    router
        .layer(middleware::from_fn(request_id_middleware))
        .layer(trace_layer)
        .layer(cors)
        .with_state(state)
}

async fn serve_openapi_spec() -> impl IntoResponse {
    let spec = tokio::fs::read_to_string("openapi.json").await.unwrap();
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        spec,
    )
}
