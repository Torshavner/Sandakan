use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;

use sandakan::application::services::{IngestionService, RetrievalService};
use sandakan::infrastructure::llm::OpenAiClient;
use sandakan::infrastructure::observability::{TracingConfig, init_tracing};
use sandakan::infrastructure::persistence::QdrantAdapter;
use sandakan::presentation::{AppState, ScaffoldConfig, create_router};

struct StubFileLoader;

#[async_trait::async_trait]
impl sandakan::application::ports::FileLoader for StubFileLoader {
    async fn extract_text(
        &self,
        data: &[u8],
        _document: &sandakan::domain::Document,
    ) -> Result<String, sandakan::application::ports::FileLoaderError> {
        String::from_utf8(data.to_vec()).map_err(|e| {
            sandakan::application::ports::FileLoaderError::ExtractionFailed(e.to_string())
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let port: u16 = std::env::var("SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    init_tracing(TracingConfig::default(), port);

    let file_loader = Arc::new(StubFileLoader);
    let llm_client = Arc::new(OpenAiClient::new(
        std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        "text-embedding-3-small".to_string(),
        "gpt-4o-mini".to_string(),
    ));
    let qdrant_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());
    let vector_store = Arc::new(
        QdrantAdapter::new(&qdrant_url, "rag_chunks".to_string())
            .await
            .expect("Failed to connect to Qdrant"),
    );

    let ingestion_service = Arc::new(IngestionService::new(
        Arc::clone(&file_loader),
        Arc::clone(&llm_client),
        Arc::clone(&vector_store),
        512,
        50,
    ));

    let retrieval_service = Arc::new(RetrievalService::new(
        Arc::clone(&llm_client),
        Arc::clone(&vector_store),
        5,
    ));

    let state = AppState {
        ingestion_service,
        retrieval_service,
        scaffold_config: ScaffoldConfig::default(),
    };

    let router = create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
