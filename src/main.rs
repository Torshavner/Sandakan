use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use config::Environment as EnvironmentSource;
use config::{Config, File};
use tokio::net::TcpListener;

use sandakan::application::services::{IngestionService, RetrievalService};
use sandakan::infrastructure::llm::OpenAiClient;
use sandakan::infrastructure::observability::{TracingConfig, init_tracing};
use sandakan::infrastructure::persistence::QdrantAdapter;
use sandakan::presentation::{AppState, Environment, ScaffoldConfig, Settings, create_router};

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
    dotenvy::dotenv().ok();

    let environment: Environment = env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");

    let configuration = Config::builder()
        .add_source(
            File::with_name(&format!("appsettings.{}", environment.as_str())).required(false),
        )
        .add_source(
            EnvironmentSource::with_prefix("APP")
                .separator("_")
                .list_separator(" "),
        )
        .build()?;

    let settings: Settings = configuration.try_deserialize()?;

    let tracing_config = TracingConfig::default();
    init_tracing(tracing_config, settings.server.port);

    tracing::info!("Application starting in {} mode", environment);

    let file_loader = Arc::new(StubFileLoader);
    let llm_client = Arc::new(OpenAiClient::new(
        settings.llm.api_key.clone(),
        settings.embeddings.model.clone(),
        settings.llm.chat_model.clone(),
    ));
    let vector_store = Arc::new(
        QdrantAdapter::new(
            &settings.qdrant.url,
            settings.qdrant.collection_name.clone(),
        )
        .await
        .expect("Failed to connect to Qdrant"),
    );

    let ingestion_service = Arc::new(IngestionService::new(
        Arc::clone(&file_loader),
        Arc::clone(&llm_client),
        Arc::clone(&vector_store),
        settings.chunking.max_chunk_size,
        settings.chunking.overlap_tokens,
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

    let addr = SocketAddr::from((
        settings
            .server
            .host
            .parse::<std::net::IpAddr>()
            .unwrap_or_else(|_| std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))),
        settings.server.port,
    ));
    tracing::info!("Listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
