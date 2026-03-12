// @AI-BYPASS-LENGTH
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use config::Environment as EnvironmentSource;
use config::{Config, File};
use sqlx::PgPool;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use sandakan::application::ports::McpClientPort;
use sandakan::application::ports::RagSourceCollector;
use sandakan::application::ports::RetrievalServicePort;
use sandakan::application::ports::{
    AudioDecoder, CollectionConfig, ConversationRepository, Embedder, EvalEventRepository,
    EvalOutboxRepository, EvalResultRepository, FileLoader, JobRepository, LlmClient,
    SparseEmbedder, StagingStore, TranscriptionEngine, VectorStore,
};
use sandakan::application::services::{
    AgentService, AgentServicePort, EvalWorker, IngestionService, IngestionWorker, RetrievalService,
};
use sandakan::domain::ContentType;
use sandakan::infrastructure::audio::{
    FfmpegAudioDecoder, TranscriptionEngineFactory, TranscriptionProvider, check_ffmpeg_binary,
};
use sandakan::infrastructure::llm::{
    EmbedderFactory, StreamingLlmClient, create_streaming_llm_client,
};
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
    Bm25SparseEmbedder, CompositeFileLoader, ExtractorFactory, PlainTextAdapter,
    TextSplitterFactory, TextSplitters,
};
use sandakan::infrastructure::tools::{
    GetFunctionSignaturesTool, InMemoryRagSourceCollector, ListDirectoryTool, NotificationAdapter,
    NotificationConfig, NotificationFormat, RagSearchAdapter, ReadFileTool, SearchFilesTool,
    SemanticToolRegistry, StaticToolRegistry, WebSearchAdapter, WebSearchConfig, build_fs_tools,
};
use sandakan::presentation::config::ReflectionSettings;
use sandakan::presentation::config::{NotificationFormat as ConfigNotificationFormat, ToolConfig};
use sandakan::presentation::{
    AppState, Environment, Settings, TranscriptionProviderSetting, create_router,
};

const INGESTION_CHANNEL_CAPACITY: usize = 64;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (environment, settings) = load_settings()?;

    let tracing_config = TracingConfig {
        environment: std::env::var("APP_ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
        json_format: settings.logging.enable_json,
        tempo_endpoint: settings.logging.tempo_endpoint.clone(),
    };
    let otel_provider = init_tracing(tracing_config, settings.server.port);

    tracing::info!("Application starting in {} mode", environment);

    let pg_pool = init_database(&settings).await?;

    let (job_repository, conversation_repository) = build_repositories(&pg_pool);
    let file_loader = build_file_loader(&settings)?;
    let embedder = build_embedder(&settings)?;
    let llm_client = build_llm_client(&settings)?;
    let vector_store = build_vector_store(&settings).await?;
    let splitters = build_text_splitters(&settings)?;
    let transcription_engine = build_transcription_engine(&settings)?;
    let staging_store = build_staging_store(&settings)?;

    let sparse_embedder: Option<Arc<dyn SparseEmbedder>> = if settings.qdrant.hybrid_search {
        Some(Arc::new(Bm25SparseEmbedder::new()))
    } else {
        None
    };

    let (eval_event_repo, eval_outbox_repo) = build_eval_repos(&settings, &pg_pool);
    let model_config = format!("{}/{}", settings.llm.provider, settings.llm.chat_model);

    let retrieval_service = Arc::new(RetrievalService::new(
        Arc::clone(&embedder),
        Arc::clone(&llm_client),
        Arc::clone(&vector_store),
        Arc::clone(&conversation_repository),
        eval_event_repo.clone(),
        eval_outbox_repo.clone(),
        sparse_embedder.clone(),
        model_config.clone(),
        settings.rag.top_k,
        settings.rag.similarity_threshold,
        settings.rag.max_context_tokens,
        settings.rag.fallback_message.clone(),
    ));

    let ingestion_service = Arc::new(IngestionService::new(
        Arc::clone(&file_loader),
        Arc::clone(&embedder),
        Arc::clone(&vector_store),
        Arc::clone(&splitters.text),
        Arc::clone(&splitters.markdown),
        Arc::clone(&job_repository),
        sparse_embedder.clone(),
    ));

    let (ingestion_sender, ingestion_receiver) = mpsc::channel(INGESTION_CHANNEL_CAPACITY);

    let mut ingestion_worker = IngestionWorker::new(
        ingestion_receiver,
        Arc::clone(&file_loader),
        Arc::clone(&embedder),
        Arc::clone(&vector_store),
        splitters.text,
        splitters.markdown,
        Arc::clone(&job_repository),
        transcription_engine,
        Arc::clone(&staging_store),
    );
    if let Some(sparse) = sparse_embedder {
        ingestion_worker = ingestion_worker.with_sparse_embedder(sparse);
    }

    let agent_eval_event_repo = eval_event_repo.clone();
    let agent_eval_outbox_repo = eval_outbox_repo.clone();

    spawn_workers(
        &settings,
        ingestion_worker,
        eval_event_repo,
        eval_outbox_repo,
        &llm_client,
        &model_config,
        &pg_pool,
    );

    let agent_service = build_agent_service(
        &settings,
        &llm_client,
        &embedder,
        &retrieval_service,
        &conversation_repository,
        agent_eval_event_repo,
        agent_eval_outbox_repo,
    )
    .await?;

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
    let addr = parse_listen_addr(&settings);

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

fn load_settings() -> anyhow::Result<(Environment, Settings)> {
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
                .prefix_separator("_")
                .separator("__")
                .list_separator(" "),
        )
        .build()?;

    let settings: Settings = configuration.try_deserialize()?;
    Ok((environment, settings))
}

async fn init_database(settings: &Settings) -> anyhow::Result<PgPool> {
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

    Ok(pg_pool)
}

fn build_repositories(
    pg_pool: &PgPool,
) -> (Arc<dyn JobRepository>, Arc<dyn ConversationRepository>) {
    let job_repository: Arc<dyn JobRepository> = Arc::new(PgJobRepository::new(pg_pool.clone()));
    let conversation_repository: Arc<dyn ConversationRepository> =
        Arc::new(PgConversationRepository::new(pg_pool.clone()));
    (job_repository, conversation_repository)
}

fn build_file_loader(settings: &Settings) -> anyhow::Result<Arc<CompositeFileLoader>> {
    let pdf_adapter: Arc<dyn FileLoader> = ExtractorFactory::create(&settings.extraction.pdf)
        .expect("Failed to initialize PDF extractor");

    tracing::info!(
        provider = ?settings.extraction.pdf.provider,
        "PDF extractor initialized"
    );

    let text_adapter: Arc<dyn FileLoader> = Arc::new(PlainTextAdapter);
    Ok(Arc::new(CompositeFileLoader::new(vec![
        (ContentType::Pdf, pdf_adapter),
        (ContentType::Text, text_adapter),
    ])))
}

fn build_embedder(settings: &Settings) -> anyhow::Result<Arc<dyn Embedder>> {
    let embedder = EmbedderFactory::create(
        settings.embeddings.provider,
        settings.embeddings.model.clone(),
        Some(settings.llm.api_key.clone()),
    )
    .expect("Failed to initialize embedder");
    Ok(embedder)
}

fn build_llm_client(settings: &Settings) -> anyhow::Result<Arc<StreamingLlmClient>> {
    let client = Arc::new(
        create_streaming_llm_client(&settings.llm, settings.rag.system_prompt.clone())
            .expect("Failed to initialize LLM client"),
    );
    tracing::info!(
        provider = %settings.llm.provider,
        model = %settings.llm.chat_model,
        "LLM client initialized"
    );
    Ok(client)
}

async fn build_vector_store(settings: &Settings) -> anyhow::Result<Arc<QdrantAdapter>> {
    let vector_store = Arc::new(
        QdrantAdapter::new(
            &settings.qdrant.url,
            settings.qdrant.collection_name.clone(),
        )
        .await
        .expect("Failed to connect to Qdrant"),
    );

    let mut collection_config = CollectionConfig::new(settings.embeddings.dimension as u64);
    if settings.qdrant.hybrid_search {
        collection_config = collection_config.with_hybrid();
        tracing::info!("Hybrid search enabled — collection will use dense + sparse vectors");
    }

    match vector_store.get_collection_vector_size().await {
        Ok(Some(existing_size)) => {
            if existing_size != collection_config.vector_dimensions {
                panic!(
                    "Dimension mismatch: Qdrant collection has {} dims but embedder config has {}",
                    existing_size, collection_config.vector_dimensions
                );
            }

            let is_hybrid = vector_store.is_hybrid_collection().await.unwrap_or(false);
            if collection_config.hybrid && !is_hybrid {
                tracing::warn!(
                    "Hybrid search enabled but collection uses dense-only schema — recreating"
                );
                vector_store
                    .delete_collection()
                    .await
                    .expect("Failed to delete incompatible collection");
                vector_store
                    .create_collection(&collection_config)
                    .await
                    .expect("Failed to recreate collection with hybrid schema");
                tracing::info!("Collection recreated with hybrid (dense + sparse) vectors");
            } else if !collection_config.hybrid && is_hybrid {
                tracing::warn!(
                    "Hybrid search disabled but collection uses hybrid schema — recreating"
                );
                vector_store
                    .delete_collection()
                    .await
                    .expect("Failed to delete incompatible collection");
                vector_store
                    .create_collection(&collection_config)
                    .await
                    .expect("Failed to recreate collection with dense-only schema");
                tracing::info!("Collection recreated with dense-only vectors");
            } else {
                tracing::info!(
                    dimension = existing_size,
                    hybrid = is_hybrid,
                    "Collection validated"
                );
            }
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

    Ok(vector_store)
}

fn build_text_splitters(settings: &Settings) -> anyhow::Result<TextSplitters> {
    TextSplitterFactory::create(
        settings.chunking.strategy,
        settings.chunking.max_chunk_size,
        settings.chunking.overlap_tokens,
    )
    .map_err(Into::into)
}

fn build_transcription_engine(settings: &Settings) -> anyhow::Result<Arc<dyn TranscriptionEngine>> {
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

    let engine = TranscriptionEngineFactory::create(
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
        settings.extraction.audio.asr_corrections.clone(),
    )
    .expect("Failed to initialize transcription engine");

    tracing::info!(
        provider = ?transcription_provider,
        model = %settings.extraction.audio.whisper_model,
        "Transcription engine initialized"
    );

    Ok(engine)
}

fn build_staging_store(settings: &Settings) -> anyhow::Result<Arc<dyn StagingStore>> {
    let store =
        StagingStoreFactory::create(&settings.storage).expect("Failed to initialize staging store");

    tracing::info!(
        provider = ?settings.storage.provider,
        "Staging store initialized"
    );

    Ok(store)
}

#[allow(clippy::type_complexity)]
fn build_eval_repos(
    settings: &Settings,
    pg_pool: &PgPool,
) -> (
    Option<Arc<dyn EvalEventRepository>>,
    Option<Arc<dyn EvalOutboxRepository>>,
) {
    if settings.eval.enabled {
        let event_repo: Arc<dyn EvalEventRepository> =
            Arc::new(PgEvalEventRepository::new(pg_pool.clone()));
        let outbox_repo: Arc<dyn EvalOutboxRepository> =
            Arc::new(PgEvalOutboxRepository::new(pg_pool.clone()));
        (Some(event_repo), Some(outbox_repo))
    } else {
        (None, None)
    }
}

fn spawn_workers(
    settings: &Settings,
    ingestion_worker: IngestionWorker<CompositeFileLoader, QdrantAdapter>,
    eval_event_repo: Option<Arc<dyn EvalEventRepository>>,
    eval_outbox_repo: Option<Arc<dyn EvalOutboxRepository>>,
    llm_client: &Arc<StreamingLlmClient>,
    model_config: &str,
    pg_pool: &PgPool,
) {
    let ingestion_worker =
        if let (Some(event_repo), Some(outbox_repo)) = (eval_event_repo, eval_outbox_repo) {
            let result_repo: Arc<dyn EvalResultRepository> =
                Arc::new(PgEvalResultRepository::new(pg_pool.clone()));

            let eval_worker = EvalWorker::new(
                outbox_repo.clone(),
                event_repo.clone(),
                result_repo,
                llm_client.clone() as Arc<dyn LlmClient>,
                settings.eval.faithfulness_threshold,
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

            ingestion_worker.with_eval(event_repo, outbox_repo, model_config)
        } else {
            tracing::info!("Eval feature disabled");
            ingestion_worker
        };

    tokio::spawn(async move {
        ingestion_worker.run().await;
    });
    tracing::info!("Ingestion worker spawned");
}

#[allow(clippy::too_many_arguments)]
async fn build_agent_service(
    settings: &Settings,
    llm_client: &Arc<StreamingLlmClient>,
    embedder: &Arc<dyn Embedder>,
    retrieval_service: &Arc<RetrievalService<StreamingLlmClient, QdrantAdapter>>,
    conversation_repository: &Arc<dyn ConversationRepository>,
    eval_event_repo: Option<Arc<dyn EvalEventRepository>>,
    eval_outbox_repo: Option<Arc<dyn EvalOutboxRepository>>,
) -> anyhow::Result<Option<Arc<dyn AgentServicePort>>> {
    if !settings.agent.enabled {
        tracing::info!("Agent feature disabled");
        return Ok(None);
    }

    let mut handlers: Vec<Arc<dyn ToolHandler>> = Vec::new();
    let mut schemas = Vec::new();
    let mut wire_clients: Vec<Arc<dyn McpClientPort>> = Vec::new();

    let has_rag_search = settings
        .agent
        .tools
        .iter()
        .any(|t| matches!(t, ToolConfig::RagSearch));
    let rag_source_collector: Option<Arc<dyn RagSourceCollector>> =
        if has_rag_search && settings.eval.enabled {
            Some(Arc::new(InMemoryRagSourceCollector::new()))
        } else {
            None
        };

    for tool in &settings.agent.tools {
        match tool {
            ToolConfig::RagSearch => {
                let rag = Arc::new(RagSearchAdapter::new(
                    Arc::clone(retrieval_service) as Arc<dyn RetrievalServicePort>,
                    rag_source_collector.clone(),
                ));
                schemas.push(RagSearchAdapter::tool_schema());
                handlers.push(rag as Arc<dyn ToolHandler>);
                tracing::info!("RAG search tool registered");
            }
            ToolConfig::WebSearch(cfg) => {
                let adapter = Arc::new(WebSearchAdapter::new(WebSearchConfig {
                    api_key: cfg.api_key.clone(),
                    endpoint: cfg.endpoint.clone(),
                    max_results: cfg.max_results,
                }));
                schemas.push(WebSearchAdapter::tool_schema());
                handlers.push(adapter as Arc<dyn ToolHandler>);
                tracing::info!("Web search tool registered");
            }
            ToolConfig::Notification(cfg) => {
                let format = match cfg.format {
                    ConfigNotificationFormat::Plain => NotificationFormat::Plain,
                    ConfigNotificationFormat::Slack => NotificationFormat::Slack,
                };
                match NotificationAdapter::new(NotificationConfig {
                    webhook_url: cfg.webhook_url.clone(),
                    format,
                    timeout_secs: cfg.timeout_secs,
                }) {
                    Ok(adapter) => {
                        schemas.push(NotificationAdapter::tool_schema());
                        handlers.push(Arc::new(adapter) as Arc<dyn ToolHandler>);
                        tracing::info!(webhook_url = %cfg.webhook_url, "Notification tool registered");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to initialize notification tool");
                    }
                }
            }
            ToolConfig::Fs(cfg) => {
                match build_fs_tools(&cfg.root_path, cfg.max_read_bytes, cfg.max_dir_entries) {
                    Ok((list_tool, read_tool, search_tool, sig_tool)) => {
                        schemas.push(ListDirectoryTool::tool_schema());
                        schemas.push(ReadFileTool::tool_schema());
                        schemas.push(SearchFilesTool::tool_schema());
                        schemas.push(GetFunctionSignaturesTool::tool_schema());
                        handlers.push(Arc::new(list_tool) as Arc<dyn ToolHandler>);
                        handlers.push(Arc::new(read_tool) as Arc<dyn ToolHandler>);
                        handlers.push(Arc::new(search_tool) as Arc<dyn ToolHandler>);
                        handlers.push(Arc::new(sig_tool) as Arc<dyn ToolHandler>);
                        tracing::info!(root_path = %cfg.root_path, "Filesystem tools registered");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to initialize fs tools");
                    }
                }
            }
            ToolConfig::McpStdio(cfg) => {
                tracing::info!(name = %cfg.name, command = %cfg.command, "Connecting to stdio MCP server");
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
            ToolConfig::McpSse(cfg) => {
                tracing::info!(name = %cfg.name, endpoint = %cfg.endpoint, "Connecting to SSE MCP server");
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

    let mcp_client: Arc<dyn McpClientPort> = if wire_clients.is_empty() {
        Arc::new(StandardMcpAdapter::new(handlers))
    } else {
        Arc::new(CompositeMcpClient::new(
            wire_clients,
            StandardMcpAdapter::new(handlers),
        ))
    };

    let tool_registry: Arc<dyn sandakan::application::ports::ToolRegistry> = if settings
        .agent
        .semantic_tools
    {
        match SemanticToolRegistry::try_new(schemas.clone(), Arc::clone(embedder)).await {
            Ok(r) => Arc::new(r),
            Err(e) => {
                tracing::warn!(error = %e, "Semantic tool registry init failed; falling back to static");
                Arc::new(StaticToolRegistry::new(schemas))
            }
        }
    } else {
        Arc::new(StaticToolRegistry::new(schemas))
    };

    let agent_config = sandakan::presentation::config::AgentServiceConfig {
        model_config: format!("{}/{}", settings.llm.provider, settings.llm.chat_model),
        max_iterations: settings.agent.max_iterations,
        tool_timeout_secs: settings.agent.tool_timeout_secs,
        tool_fail_fast: settings.agent.tool_fail_fast,
        system_prompt: settings.agent.system_prompt.clone(),
        reflection: ReflectionSettings {
            enabled: settings.agent.reflection.enabled,
            score_threshold: settings.agent.reflection.score_threshold,
            correction_budget: settings.agent.reflection.correction_budget,
            critic_system_prompt: settings.agent.reflection.critic_system_prompt.clone(),
        },
        max_tool_results: settings.agent.max_tool_results,
        dynamic_tools_description: settings.agent.dynamic_tools_description,
        max_context_tokens: settings.agent.max_context_tokens,
        smart_pruning: settings.agent.smart_pruning,
    };

    let svc = Arc::new(AgentService::new(
        llm_client.clone() as Arc<dyn LlmClient>,
        mcp_client,
        tool_registry,
        Arc::clone(conversation_repository),
        eval_event_repo,
        eval_outbox_repo,
        rag_source_collector,
        agent_config,
    ));

    tracing::info!(
        max_iterations = settings.agent.max_iterations,
        tool_timeout_secs = settings.agent.tool_timeout_secs,
        tool_fail_fast = settings.agent.tool_fail_fast,
        "AgentService initialized"
    );

    Ok(Some(svc as Arc<dyn AgentServicePort>))
}

fn parse_listen_addr(settings: &Settings) -> SocketAddr {
    SocketAddr::from((
        settings
            .server
            .host
            .parse::<std::net::IpAddr>()
            .unwrap_or_else(|_| std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))),
        settings.server.port,
    ))
}
