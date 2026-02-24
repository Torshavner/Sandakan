use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use config::Environment as EnvironmentSource;
use config::{Config, File};
use tokio::net::TcpListener;

use sandakan::application::ports::McpClientPort;
use sandakan::application::ports::RetrievalServicePort;
use sandakan::application::ports::{
    AudioDecoder, CollectionConfig, ConversationRepository, EvalEventRepository,
    EvalOutboxRepository, EvalResultRepository, FileLoader, JobRepository, LlmClient, VectorStore,
};
use sandakan::application::services::{
    AgentService, AgentServicePort, EvalWorker, IngestionService, IngestionWorker, RetrievalService,
};
use sandakan::domain::ContentType;
use sandakan::infrastructure::audio::{
    FfmpegAudioDecoder, TranscriptionEngineFactory, TranscriptionProvider, check_ffmpeg_binary,
};
use sandakan::infrastructure::llm::{EmbedderFactory, create_streaming_llm_client};
use sandakan::infrastructure::mcp::{
    CompositeMcpClient, SseMcpClient, StandardMcpAdapter, StdioMcpClient, ToolHandler,
};
use sandakan::infrastructure::observability::{TracingConfig, init_tracing};
use sandakan::infrastructure::persistence::{
    PgConversationRepository, PgEvalEventRepository, PgEvalOutboxRepository,
    PgEvalResultRepository, PgJobRepository, QdrantAdapter, create_pool,
};
use sandakan::infrastructure::storage::StagingStoreFactory;
use sandakan::infrastructure::text_processing::{
    CompositeFileLoader, ExtractorFactory, PlainTextAdapter, TextSplitterFactory,
};
use sandakan::infrastructure::tools::{
    NotificationAdapter, NotificationConfig, NotificationFormat, RagSearchAdapter,
    StaticToolRegistry, WebSearchAdapter, WebSearchConfig,
};
use sandakan::presentation::{
    AppState, Environment, McpServerConfig, NotificationFormatSetting, Settings,
    TranscriptionProviderSetting, create_router,
};

const INGESTION_CHANNEL_CAPACITY: usize = 64;

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

    let tracing_config = TracingConfig {
        environment: std::env::var("APP_ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
        json_format: settings.logging.enable_json,
        tempo_endpoint: settings.logging.tempo_endpoint.clone(),
    };
    let otel_provider = init_tracing(tracing_config, settings.server.port);

    tracing::info!("Application starting in {} mode", environment);

    let pg_pool = create_pool(&settings.database.url, settings.database.max_connections)
        .await
        .expect("Failed to create PostgreSQL connection pool");

    if settings.database.run_migrations {
        sqlx::migrate!()
            .run(&pg_pool)
            .await
            .expect("Failed to run database migrations");
        tracing::info!("Database migrations completed");
    }

    let job_repository: Arc<dyn JobRepository> = Arc::new(PgJobRepository::new(pg_pool.clone()));
    let conversation_repository: Arc<dyn ConversationRepository> =
        Arc::new(PgConversationRepository::new(pg_pool.clone()));

    let pdf_adapter: Arc<dyn FileLoader> = ExtractorFactory::create(&settings.extraction.pdf)
        .expect("Failed to initialize PDF extractor");

    tracing::info!(
        provider = ?settings.extraction.pdf.provider,
        "PDF extractor initialized"
    );

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

    let llm_client = Arc::new(
        create_streaming_llm_client(&settings.llm, settings.rag.system_prompt.clone())
            .expect("Failed to initialize LLM client"),
    );
    tracing::info!(
        provider = %settings.llm.provider,
        model = %settings.llm.chat_model,
        "LLM client initialized"
    );

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
        settings.chunking.strategy,
        settings.chunking.max_chunk_size,
        settings.chunking.overlap_tokens,
    );

    let transcription_provider = match settings.extraction.audio.provider {
        TranscriptionProviderSetting::Local => TranscriptionProvider::Local,
        TranscriptionProviderSetting::OpenAi => TranscriptionProvider::OpenAi,
        TranscriptionProviderSetting::Azure => TranscriptionProvider::Azure,
    };

    let audio_decoder: Option<Arc<dyn AudioDecoder>> =
        if matches!(transcription_provider, TranscriptionProvider::Local) {
            check_ffmpeg_binary()
                .expect("ffmpeg required for Local transcription provider — install ffmpeg");
            tracing::info!("ffmpeg binary validated");
            Some(Arc::new(FfmpegAudioDecoder))
        } else {
            None
        };

    let transcription_engine = TranscriptionEngineFactory::create(
        transcription_provider,
        &settings.extraction.audio.whisper_model,
        Some(settings.llm.api_key.clone()),
        settings
            .extraction
            .audio
            .azure_endpoint
            .clone()
            .or(settings.llm.base_url.clone()),
        settings.extraction.audio.azure_deployment.clone(),
        settings.extraction.audio.azure_api_version.clone(),
        audio_decoder,
    )
    .expect("Failed to initialize transcription engine");

    tracing::info!(
        provider = ?transcription_provider,
        model = %settings.extraction.audio.whisper_model,
        "Transcription engine initialized"
    );

    let staging_store =
        StagingStoreFactory::create(&settings.storage).expect("Failed to initialize staging store");

    tracing::info!(
        provider = ?settings.storage.provider,
        "Staging store initialized"
    );

    let (ingestion_sender, ingestion_receiver) =
        tokio::sync::mpsc::channel(INGESTION_CHANNEL_CAPACITY);

    let worker = IngestionWorker::new(
        ingestion_receiver,
        Arc::clone(&file_loader),
        Arc::clone(&embedder),
        Arc::clone(&vector_store),
        text_splitter.clone(),
        Arc::clone(&job_repository),
        transcription_engine,
        Arc::clone(&staging_store),
    );

    tokio::spawn(async move {
        worker.run().await;
    });
    tracing::info!("Ingestion worker spawned");

    let ingestion_service = Arc::new(IngestionService::new(
        Arc::clone(&file_loader),
        Arc::clone(&embedder),
        Arc::clone(&vector_store),
        text_splitter,
        Arc::clone(&job_repository),
    ));

    let (eval_event_repo, eval_outbox_repo) = if settings.eval.enabled {
        let event_repo: Arc<dyn EvalEventRepository> =
            Arc::new(PgEvalEventRepository::new(pg_pool.clone()));
        let outbox_repo: Arc<dyn EvalOutboxRepository> =
            Arc::new(PgEvalOutboxRepository::new(pg_pool.clone()));
        (Some(event_repo), Some(outbox_repo))
    } else {
        (None, None)
    };

    let model_config = format!("{}/{}", settings.llm.provider, settings.llm.chat_model);

    let retrieval_service = Arc::new(RetrievalService::new(
        Arc::clone(&embedder),
        Arc::clone(&llm_client),
        Arc::clone(&vector_store),
        Arc::clone(&conversation_repository),
        eval_event_repo.clone(),
        eval_outbox_repo.clone(),
        model_config,
        settings.rag.top_k,
        settings.rag.similarity_threshold,
        settings.rag.max_context_tokens,
        settings.rag.fallback_message.clone(),
    ));

    let agent_eval_event_repo = eval_event_repo.clone();
    let agent_eval_outbox_repo = eval_outbox_repo.clone();

    if let (Some(event_repo), Some(outbox_repo)) = (eval_event_repo, eval_outbox_repo) {
        let result_repo: Arc<dyn EvalResultRepository> =
            Arc::new(PgEvalResultRepository::new(pg_pool.clone()));

        let eval_worker = EvalWorker::new(
            outbox_repo,
            event_repo,
            result_repo,
            Arc::clone(&embedder),
            llm_client.clone() as Arc<dyn LlmClient>,
            settings.eval.faithfulness_threshold,
            settings.eval.correctness_threshold,
            std::time::Duration::from_secs(settings.eval.worker_poll_interval_secs),
            settings.eval.worker_batch_size,
        );
        tokio::spawn(async move {
            eval_worker.run().await;
        });
        tracing::info!(
            poll_interval_secs = settings.eval.worker_poll_interval_secs,
            batch_size = settings.eval.worker_batch_size,
            "EvalWorker spawned"
        );
    } else {
        tracing::info!("Eval feature disabled");
    }

    let agent_service: Option<Arc<dyn AgentServicePort>> = if settings.agent.enabled {
        let mut handlers: Vec<Arc<dyn ToolHandler>> = Vec::new();
        let mut schemas = Vec::new();

        if let Some(ws_config) = &settings.agent.web_search {
            let adapter = Arc::new(WebSearchAdapter::new(WebSearchConfig {
                api_key: ws_config.api_key.clone(),
                endpoint: ws_config.endpoint.clone(),
                max_results: ws_config.max_results,
            }));
            schemas.push(WebSearchAdapter::tool_schema());
            handlers.push(adapter as Arc<dyn ToolHandler>);
        }

        if settings.agent.rag_search_enabled {
            let rag = Arc::new(RagSearchAdapter::new(
                Arc::clone(&retrieval_service) as Arc<dyn RetrievalServicePort>
            ));
            schemas.push(RagSearchAdapter::tool_schema());
            handlers.push(rag as Arc<dyn ToolHandler>);
            tracing::info!("RAG search tool registered");
        }

        if let Some(notif) = &settings.agent.notification {
            let format = match notif.format {
                NotificationFormatSetting::Plain => NotificationFormat::Plain,
                NotificationFormatSetting::Slack => NotificationFormat::Slack,
            };
            let adapter = Arc::new(NotificationAdapter::new(NotificationConfig {
                webhook_url: notif.webhook_url.clone(),
                format,
                timeout_secs: notif.timeout_secs,
            }));
            schemas.push(NotificationAdapter::tool_schema());
            handlers.push(adapter as Arc<dyn ToolHandler>);
            tracing::info!(
                webhook_url = %notif.webhook_url,
                "Notification webhook tool registered"
            );
        }

        // Build a composite MCP client: one entry per configured wire server.
        // Each wire client is wrapped in a `WireMcpRouter` that tries it before
        // falling back to `StandardMcpAdapter` (for compiled-in handlers).
        let mut wire_clients: Vec<Arc<dyn McpClientPort>> = Vec::new();

        for server_cfg in &settings.agent.mcp_servers {
            match server_cfg {
                McpServerConfig::Stdio(cfg) => {
                    tracing::info!(
                        name = %cfg.name,
                        command = %cfg.command,
                        "Connecting to stdio MCP server"
                    );
                    match StdioMcpClient::new(&cfg.command, &cfg.args, &cfg.env).await {
                        Ok(client) => {
                            schemas.extend(client.tool_schemas.clone());
                            wire_clients.push(Arc::new(client) as Arc<dyn McpClientPort>);
                        }
                        Err(e) => {
                            tracing::error!(name = %cfg.name, error = %e, "Failed to start stdio MCP server");
                        }
                    }
                }
                McpServerConfig::Sse(cfg) => {
                    tracing::info!(
                        name = %cfg.name,
                        endpoint = %cfg.endpoint,
                        "Connecting to SSE MCP server"
                    );
                    match SseMcpClient::new(&cfg.endpoint).await {
                        Ok(client) => {
                            schemas.extend(client.tool_schemas.clone());
                            wire_clients.push(Arc::new(client) as Arc<dyn McpClientPort>);
                        }
                        Err(e) => {
                            tracing::error!(name = %cfg.name, error = %e, "Failed to connect to SSE MCP server");
                        }
                    }
                }
            }
        }

        // If wire servers are configured, use a composite router that tries each
        // in order before the compiled-in `StandardMcpAdapter`.
        let mcp_client: Arc<dyn McpClientPort> = if wire_clients.is_empty() {
            Arc::new(StandardMcpAdapter::new(handlers)) as Arc<dyn McpClientPort>
        } else {
            Arc::new(CompositeMcpClient::new(
                wire_clients,
                StandardMcpAdapter::new(handlers),
            )) as Arc<dyn McpClientPort>
        };

        let tool_registry = Arc::new(StaticToolRegistry::new(schemas));
        let agent_model_config = format!("{}/{}", settings.llm.provider, settings.llm.chat_model);

        let svc = Arc::new(AgentService::new(
            llm_client.clone() as Arc<dyn LlmClient>,
            mcp_client,
            tool_registry,
            Arc::clone(&conversation_repository),
            agent_eval_event_repo,
            agent_eval_outbox_repo,
            agent_model_config,
            settings.agent.max_iterations,
        ));

        tracing::info!(
            max_iterations = settings.agent.max_iterations,
            "AgentService initialized"
        );
        Some(svc as Arc<dyn AgentServicePort>)
    } else {
        tracing::info!("Agent feature disabled");
        None
    };

    let state = AppState {
        ingestion_service,
        retrieval_service,
        conversation_repository,
        job_repository,
        ingestion_sender,
        staging_store,
        agent_service,
        settings: settings.clone(),
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

    if let Some(provider) = otel_provider {
        if let Err(e) = provider.shutdown() {
            tracing::warn!(error = %e, "OTel provider shutdown error");
        }
    }

    Ok(())
}
