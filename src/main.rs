use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use config::Environment as EnvironmentSource;
use config::{Config, File};
use tokio::net::TcpListener;

use sandakan::application::ports::{CollectionConfig, FileLoader, VectorStore};
use sandakan::application::services::{IngestionService, RetrievalService};
use sandakan::domain::ContentType;
use sandakan::infrastructure::llm::{EmbedderFactory, OpenAiClient};
use sandakan::infrastructure::observability::{TracingConfig, init_tracing};
use sandakan::infrastructure::persistence::QdrantAdapter;
use sandakan::infrastructure::text_processing::{
    CompositeFileLoader, PdfAdapter, PlainTextAdapter, TextSplitterFactory,
};
use sandakan::presentation::{AppState, Environment, ScaffoldConfig, Settings, create_router};

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

    let pdf_adapter: Arc<dyn FileLoader> = Arc::new(PdfAdapter::new());
    let text_adapter: Arc<dyn FileLoader> = Arc::new(PlainTextAdapter);
    let file_loader = Arc::new(CompositeFileLoader::new(vec![
        (ContentType::Pdf, pdf_adapter),
        (ContentType::Text, text_adapter),
    ]));

    let embedder = EmbedderFactory::create(
        settings.embeddings.provider,
        settings.embeddings.model.clone(),
        Some(settings.llm.api_key.clone()),
    )
    .expect("Failed to initialize embedder");

    let llm_client = Arc::new(OpenAiClient::new(
        settings.llm.api_key.clone(),
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

    let collection_config = CollectionConfig::new(settings.embeddings.dimension as u64);

    match vector_store.get_collection_vector_size().await {
        Ok(Some(existing_size)) => {
            if existing_size != collection_config.vector_dimensions {
                panic!(
                    "Dimension mismatch: Qdrant collection has {} dims but embedder config has {}",
                    existing_size, collection_config.vector_dimensions
                );
            }
            tracing::info!(dimension = existing_size, "Collection dimension validated");
        }
        Ok(None) => {
            vector_store
                .create_collection(&collection_config)
                .await
                .expect("Failed to create collection");
            tracing::info!(
                dimension = collection_config.vector_dimensions,
                "Collection created"
            );
        }
        Err(e) => {
            tracing::warn!("Could not check collection: {}", e);
        }
    }

    let text_splitter = TextSplitterFactory::create(
        settings.embeddings.strategy,
        settings.chunking.max_chunk_size,
        settings.chunking.overlap_tokens,
    );

    let ingestion_service = Arc::new(IngestionService::new(
        Arc::clone(&file_loader),
        Arc::clone(&embedder),
        Arc::clone(&vector_store),
        text_splitter,
    ));

    let retrieval_service = Arc::new(RetrievalService::new(
        Arc::clone(&embedder),
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
