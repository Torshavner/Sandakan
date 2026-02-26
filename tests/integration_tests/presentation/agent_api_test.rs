use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use sandakan::application::ports::{Embedder, TextSplitter};
use sandakan::application::services::{
    AgentChatRequest, AgentChatResponse, AgentError, AgentProgressEvent, AgentServicePort,
    IngestionMessage, IngestionService, RetrievalService,
};
use sandakan::domain::ConversationId;
use sandakan::infrastructure::llm::{MockEmbedder, MockLlmClient};
use sandakan::infrastructure::persistence::{
    MockConversationRepository, MockJobRepository, MockVectorStore,
};
use sandakan::infrastructure::storage::MockStagingStore;
use sandakan::infrastructure::text_processing::MockFileLoader;
use sandakan::presentation::config::{AgentSettings, EvalSettings};
use sandakan::presentation::{AppState, Settings, create_router};

const TEST_CHUNK_SIZE: usize = 512;
const TEST_CHUNK_OVERLAP: usize = 50;
const TEST_TOP_K: usize = 5;
const TEST_SIMILARITY_THRESHOLD: f32 = 0.7;
const TEST_MAX_CONTEXT_TOKENS: usize = 3072;
const TEST_FALLBACK_MESSAGE: &str = "I cannot answer.";

// ─── Mock AgentServicePort ────────────────────────────────────────────────────

struct MockAgentService;

#[async_trait::async_trait]
impl AgentServicePort for MockAgentService {
    async fn chat(&self, _request: AgentChatRequest) -> Result<AgentChatResponse, AgentError> {
        use sandakan::application::ports::LlmClientError;

        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let _ = tx.send(AgentProgressEvent::Thinking { iteration: 0 }).await;
        drop(tx);

        let token_stream: sandakan::application::ports::LlmTokenStream =
            Box::pin(futures::stream::once(async {
                Ok::<String, LlmClientError>("Agent answer".to_string())
            }));

        Ok(AgentChatResponse {
            progress_rx: rx,
            token_stream,
            conversation_id: ConversationId::new(),
        })
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn test_settings() -> Settings {
    use sandakan::presentation::config::{
        AudioExtractionSettings, ChunkingSettings, ChunkingStrategy, DatabaseSettings,
        EmbeddingProvider, EmbeddingsSettings, ExtractionSettings, LlmSettings, LoggingSettings,
        PdfExtractionSettings, QdrantSettings, RagSettings, ServerSettings, StorageProviderSetting,
        StorageSettings, TranscriptionProviderSetting, VideoExtractionSettings,
    };

    Settings {
        server: ServerSettings {
            host: "127.0.0.1".to_string(),
            port: 3000,
        },
        qdrant: QdrantSettings {
            url: "http://localhost:6334".to_string(),
            collection_name: "test".to_string(),
        },
        database: DatabaseSettings {
            url: "postgres://test".to_string(),
            max_connections: 5,
            run_migrations: false,
        },
        embeddings: EmbeddingsSettings {
            provider: EmbeddingProvider::Local,
            model: "test-model".to_string(),
            dimension: 384,
            chunk_overlap: 50,
        },
        chunking: ChunkingSettings {
            max_chunk_size: 512,
            overlap_tokens: 50,
            strategy: ChunkingStrategy::Semantic,
        },
        llm: LlmSettings {
            provider: "openai".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            azure_endpoint: None,
            chat_model: "gpt-4".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            sse_keep_alive_seconds: 15,
        },
        logging: LoggingSettings {
            level: "info".to_string(),
            enable_json: false,
            enable_udp: false,
            tempo_endpoint: None,
        },
        storage: StorageSettings {
            provider: StorageProviderSetting::Local,
            local_path: "./test-uploads".to_string(),
            max_upload_size_bytes: 1073741824,
            azure_account: None,
            azure_access_key: None,
            azure_container: None,
        },
        extraction: ExtractionSettings {
            pdf: PdfExtractionSettings {
                enabled: true,
                max_file_size_mb: 50,
                provider: sandakan::presentation::config::ExtractorProvider::LocalVlm,
                vlm_model: None,
                vlm_revision: None,
                vlm_base_url: None,
                vlm_api_key: None,
                azure_endpoint: None,
                azure_key: None,
            },
            audio: AudioExtractionSettings {
                enabled: true,
                max_file_size_mb: 100,
                whisper_model: "base".to_string(),
                provider: TranscriptionProviderSetting::Local,
                azure_endpoint: None,
                azure_deployment: None,
                azure_key: None,
                azure_api_version: None,
                asr_corrections: Default::default(),
            },
            video: VideoExtractionSettings {
                enabled: true,
                max_file_size_mb: 500,
            },
        },
        rag: RagSettings {
            similarity_threshold: TEST_SIMILARITY_THRESHOLD,
            max_context_tokens: TEST_MAX_CONTEXT_TOKENS,
            top_k: TEST_TOP_K,
            system_prompt: "test prompt".to_string(),
            fallback_message: TEST_FALLBACK_MESSAGE.to_string(),
        },
        eval: EvalSettings::default(),
        agent: AgentSettings::default(),
    }
}

fn create_ingestion_sender() -> tokio::sync::mpsc::Sender<IngestionMessage> {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(16);
    tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    sender
}

fn create_test_app_with_agent(agent_service: Option<Arc<dyn AgentServicePort>>) -> axum::Router {
    use sandakan::infrastructure::text_processing::RecursiveCharacterSplitter;

    let file_loader = Arc::new(MockFileLoader);
    let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder);
    let llm_client = Arc::new(MockLlmClient);
    let vector_store = Arc::new(MockVectorStore);
    let text_splitter: Arc<dyn TextSplitter> = Arc::new(RecursiveCharacterSplitter::new(
        TEST_CHUNK_SIZE,
        TEST_CHUNK_OVERLAP,
    ));
    let markdown_splitter: Arc<dyn TextSplitter> = Arc::new(RecursiveCharacterSplitter::new(
        TEST_CHUNK_SIZE,
        TEST_CHUNK_OVERLAP,
    ));

    let ingestion_service = Arc::new(IngestionService::new(
        Arc::clone(&file_loader),
        Arc::clone(&embedder),
        Arc::clone(&vector_store),
        Arc::clone(&text_splitter),
        Arc::clone(&markdown_splitter),
        Arc::new(MockJobRepository),
    ));

    let retrieval_service = Arc::new(RetrievalService::new(
        Arc::clone(&embedder),
        Arc::clone(&llm_client),
        Arc::clone(&vector_store),
        Arc::new(MockConversationRepository),
        None,
        None,
        "test/mock-model".to_string(),
        TEST_TOP_K,
        TEST_SIMILARITY_THRESHOLD,
        TEST_MAX_CONTEXT_TOKENS,
        TEST_FALLBACK_MESSAGE.to_string(),
    ));

    let state = AppState {
        ingestion_service,
        retrieval_service,
        conversation_repository: Arc::new(MockConversationRepository),
        job_repository: Arc::new(MockJobRepository),
        ingestion_sender: create_ingestion_sender(),
        staging_store: Arc::new(MockStagingStore),
        agent_service,
        settings: test_settings(),
    };

    create_router(state)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn given_agent_enabled_when_posting_valid_message_then_returns_sse_stream() {
    let app = create_test_app_with_agent(Some(Arc::new(MockAgentService)));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/agent/chat")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"message": "What is the news today?"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.contains("text/event-stream"))
            .unwrap_or(false),
        "Expected content-type: text/event-stream"
    );
}

#[tokio::test]
async fn given_agent_disabled_when_posting_to_agent_chat_then_returns_404() {
    let app = create_test_app_with_agent(None);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/agent/chat")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"message": "hello"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn given_empty_message_when_posting_to_agent_chat_then_returns_bad_request() {
    let app = create_test_app_with_agent(Some(Arc::new(MockAgentService)));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/agent/chat")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"message": ""}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
