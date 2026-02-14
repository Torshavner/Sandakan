use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures::stream::StreamExt;
use tower::ServiceExt;

use sandakan::application::ports::{
    CollectionConfig, ConversationRepository, Embedder, EmbedderError, FileLoader, FileLoaderError,
    JobRepository, LlmClient, RepositoryError, SearchResult, VectorStore, VectorStoreError,
};
use sandakan::application::services::IngestionMessage;
use sandakan::application::services::{IngestionService, RetrievalService};
use sandakan::domain::{
    Chunk, ChunkId, Conversation, ConversationId, Document, DocumentId, Embedding, Job, JobId,
    JobStatus, Message,
};
use sandakan::infrastructure::llm::create_streaming_llm_client;
use sandakan::infrastructure::text_processing::RecursiveCharacterSplitter;
use sandakan::presentation::config::{
    AudioExtractionSettings, ChunkingSettings, DatabaseSettings, EmbeddingProvider,
    EmbeddingStrategy, EmbeddingsSettings, ExtractionSettings, LlmSettings, LoggingSettings,
    PdfExtractionSettings, QdrantSettings, RagSettings, ServerSettings,
    TranscriptionProviderSetting, VideoExtractionSettings,
};
use sandakan::presentation::{AppState, ScaffoldConfig, Settings, create_router};

const OLLAMA_BASE_URL: &str = "http://localhost:11434/v1";
const OLLAMA_MODEL: &str = "llama3.1";
const SYSTEM_PROMPT: &str = "You are a helpful assistant. Answer the user's question using ONLY the provided context.\n\nContext:\n{context}";

async fn ollama_available() -> bool {
    reqwest::Client::new()
        .get("http://localhost:11434/api/tags")
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .is_ok()
}

fn ollama_llm_settings() -> LlmSettings {
    LlmSettings {
        provider: "lmstudio".to_string(),
        api_key: "ollama".to_string(),
        base_url: Some(OLLAMA_BASE_URL.to_string()),
        azure_endpoint: None,
        chat_model: OLLAMA_MODEL.to_string(),
        max_tokens: 256,
        temperature: 0.1,
        sse_keep_alive_seconds: 15,
    }
}

// --- Mocks for non-LLM dependencies (reused from api_test pattern) ---

struct MockFileLoader;

#[async_trait::async_trait]
impl FileLoader for MockFileLoader {
    async fn extract_text(&self, data: &[u8], _doc: &Document) -> Result<String, FileLoaderError> {
        String::from_utf8(data.to_vec())
            .map_err(|e| FileLoaderError::ExtractionFailed(e.to_string()))
    }
}

struct MockEmbedder;

#[async_trait::async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, _text: &str) -> Result<Embedding, EmbedderError> {
        Ok(Embedding::new(vec![0.1; 384]))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>, EmbedderError> {
        Ok(texts
            .iter()
            .map(|_| Embedding::new(vec![0.1; 384]))
            .collect())
    }
}

struct MockVectorStore;

#[async_trait::async_trait]
impl VectorStore for MockVectorStore {
    async fn create_collection(
        &self,
        _config: &CollectionConfig,
    ) -> Result<bool, VectorStoreError> {
        Ok(true)
    }
    async fn collection_exists(&self) -> Result<bool, VectorStoreError> {
        Ok(true)
    }
    async fn get_collection_vector_size(&self) -> Result<Option<u64>, VectorStoreError> {
        Ok(Some(384))
    }
    async fn delete_collection(&self) -> Result<(), VectorStoreError> {
        Ok(())
    }
    async fn upsert(
        &self,
        _chunks: &[Chunk],
        _embeddings: &[Embedding],
    ) -> Result<(), VectorStoreError> {
        Ok(())
    }
    async fn search(
        &self,
        _embedding: &Embedding,
        _top_k: usize,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        Ok(vec![SearchResult {
            chunk: Chunk::new(
                "Rust is a systems programming language focused on safety and performance."
                    .to_string(),
                DocumentId::new(),
                Some(1),
                0,
            ),
            score: 0.95,
        }])
    }
    async fn delete(&self, _chunk_ids: &[ChunkId]) -> Result<(), VectorStoreError> {
        Ok(())
    }
}

struct MockConversationRepository;

#[async_trait::async_trait]
impl ConversationRepository for MockConversationRepository {
    async fn create_conversation(
        &self,
        _conversation: &Conversation,
    ) -> Result<(), RepositoryError> {
        Ok(())
    }
    async fn get_conversation(
        &self,
        _id: ConversationId,
    ) -> Result<Option<Conversation>, RepositoryError> {
        Ok(None)
    }
    async fn append_message(&self, _message: &Message) -> Result<(), RepositoryError> {
        Ok(())
    }
    async fn get_messages(
        &self,
        _conversation_id: ConversationId,
        _limit: usize,
    ) -> Result<Vec<Message>, RepositoryError> {
        Ok(vec![])
    }
}

struct MockJobRepository;

#[async_trait::async_trait]
impl JobRepository for MockJobRepository {
    async fn create(&self, _job: &Job) -> Result<(), RepositoryError> {
        Ok(())
    }
    async fn get_by_id(&self, _id: JobId) -> Result<Option<Job>, RepositoryError> {
        Ok(None)
    }
    async fn update_status(
        &self,
        _id: JobId,
        _status: JobStatus,
        _error_message: Option<&str>,
    ) -> Result<(), RepositoryError> {
        Ok(())
    }
    async fn list_by_status(&self, _status: JobStatus) -> Result<Vec<Job>, RepositoryError> {
        Ok(vec![])
    }
}

fn test_settings() -> Settings {
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
            strategy: EmbeddingStrategy::Semantic,
            dimension: 384,
            chunk_overlap: 50,
        },
        chunking: ChunkingSettings {
            max_chunk_size: 512,
            overlap_tokens: 50,
        },
        llm: ollama_llm_settings(),
        logging: LoggingSettings {
            level: "info".to_string(),
            enable_json: false,
            enable_udp: false,
        },
        extraction: ExtractionSettings {
            pdf: PdfExtractionSettings {
                enabled: true,
                max_file_size_mb: 50,
            },
            audio: AudioExtractionSettings {
                enabled: true,
                max_file_size_mb: 100,
                whisper_model: "base".to_string(),
                provider: TranscriptionProviderSetting::Local,
            },
            video: VideoExtractionSettings {
                enabled: true,
                max_file_size_mb: 500,
            },
        },
        rag: RagSettings {
            similarity_threshold: 0.7,
            max_context_tokens: 3072,
            top_k: 5,
            system_prompt: SYSTEM_PROMPT.to_string(),
            fallback_message: "I cannot answer this.".to_string(),
        },
    }
}

fn create_ollama_test_app() -> axum::Router {
    let settings = ollama_llm_settings();
    let llm_client = Arc::new(
        create_streaming_llm_client(&settings, SYSTEM_PROMPT.to_string())
            .expect("Failed to create Ollama LLM client"),
    );

    let file_loader = Arc::new(MockFileLoader);
    let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder);
    let vector_store = Arc::new(MockVectorStore);
    let text_splitter = Arc::new(RecursiveCharacterSplitter::new(512, 50));

    let ingestion_service = Arc::new(IngestionService::new(
        Arc::clone(&file_loader),
        Arc::clone(&embedder),
        Arc::clone(&vector_store),
        text_splitter,
        Arc::new(MockJobRepository) as Arc<dyn JobRepository>,
    ));

    let retrieval_service = Arc::new(RetrievalService::new(
        Arc::clone(&embedder),
        Arc::clone(&llm_client),
        Arc::clone(&vector_store),
        Arc::new(MockConversationRepository) as Arc<dyn ConversationRepository>,
        5,
        0.7,
        3072,
        "I cannot answer this.".to_string(),
    ));

    let (ingestion_sender, _ingestion_receiver) =
        tokio::sync::mpsc::channel::<IngestionMessage>(16);

    let state = AppState {
        ingestion_service,
        retrieval_service,
        conversation_repository: Arc::new(MockConversationRepository),
        job_repository: Arc::new(MockJobRepository) as Arc<dyn JobRepository>,
        ingestion_sender,
        settings: test_settings(),
        scaffold_config: ScaffoldConfig {
            enabled: false,
            mock_response_delay_ms: 0,
        },
    };

    create_router(state)
}

// --- Tests ---

#[tokio::test]
async fn given_ollama_available_when_complete_then_returns_non_empty_answer() {
    if !ollama_available().await {
        eprintln!("Skipping: Ollama not available at localhost:11434");
        return;
    }

    let settings = ollama_llm_settings();
    let client = create_streaming_llm_client(&settings, SYSTEM_PROMPT.to_string())
        .expect("Failed to create client");

    let answer = client
        .complete(
            "What is Rust?",
            "Rust is a systems programming language focused on safety and performance.",
        )
        .await
        .expect("complete() failed");

    assert!(!answer.is_empty(), "Answer should not be empty");
    eprintln!("Ollama complete response: {answer}");
}

#[tokio::test]
async fn given_ollama_available_when_complete_stream_then_yields_multiple_tokens() {
    if !ollama_available().await {
        eprintln!("Skipping: Ollama not available at localhost:11434");
        return;
    }

    let settings = ollama_llm_settings();
    let client = create_streaming_llm_client(&settings, SYSTEM_PROMPT.to_string())
        .expect("Failed to create client");

    let mut stream = client
        .complete_stream(
            "What is Rust?",
            "Rust is a systems programming language focused on safety and performance.",
        )
        .await
        .expect("complete_stream() failed");

    let mut tokens = Vec::new();
    while let Some(result) = stream.next().await {
        let token = result.expect("Stream yielded an error");
        tokens.push(token);
    }

    assert!(
        tokens.len() > 1,
        "Stream should yield multiple tokens, got {}",
        tokens.len()
    );
    let full_text: String = tokens.into_iter().collect();
    assert!(
        !full_text.is_empty(),
        "Concatenated stream should not be empty"
    );
    eprintln!("Ollama stream response: {full_text}");
}

#[tokio::test]
async fn given_ollama_available_when_streaming_chat_completion_then_returns_sse_events() {
    if !ollama_available().await {
        eprintln!("Skipping: Ollama not available at localhost:11434");
        return;
    }

    let app = create_ollama_test_app();

    let request_body = serde_json::json!({
        "model": "rag-pipeline",
        "messages": [{"role": "user", "content": "What is Rust?"}],
        "stream": true
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body);

    let events: Vec<&str> = body_str
        .lines()
        .filter(|line| line.starts_with("data: "))
        .collect();

    assert!(
        events.len() >= 3,
        "Should have at least start + content + done + [DONE], got {}",
        events.len()
    );
    assert!(body_str.contains("[DONE]"), "Stream should end with [DONE]");
    eprintln!("SSE events count: {}", events.len());
}

#[tokio::test]
async fn given_ollama_available_when_non_streaming_chat_completion_then_returns_json() {
    if !ollama_available().await {
        eprintln!("Skipping: Ollama not available at localhost:11434");
        return;
    }

    let app = create_ollama_test_app();

    let request_body = serde_json::json!({
        "model": "rag-pipeline",
        "messages": [{"role": "user", "content": "What is Rust?"}]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&body).expect("Response should be valid JSON");

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .expect("Should have content field");
    assert!(!content.is_empty(), "Content should not be empty");
    eprintln!("Non-streaming response: {content}");
}
