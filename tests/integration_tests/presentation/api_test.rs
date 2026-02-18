use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use sandakan::application::ports::{
    CollectionConfig, ConversationRepository, Embedder, EmbedderError, FileLoader, FileLoaderError,
    JobRepository, LlmClient, LlmClientError, RepositoryError, SearchResult, StagingStore,
    StagingStoreError, VectorStore, VectorStoreError,
};
use sandakan::application::services::{IngestionMessage, IngestionService, RetrievalService};
use sandakan::domain::{
    Chunk, ChunkId, Conversation, ConversationId, Document, DocumentId, Embedding, Job, JobId,
    JobStatus, Message,
};
use sandakan::presentation::config::{
    AudioExtractionSettings, ChunkingSettings, ChunkingStrategy, DatabaseSettings,
    EmbeddingProvider, EmbeddingsSettings, ExtractionSettings, LlmSettings, LoggingSettings,
    PdfExtractionSettings, QdrantSettings, RagSettings, ServerSettings, StorageProviderSetting,
    StorageSettings, TranscriptionProviderSetting, VideoExtractionSettings,
};
use sandakan::presentation::{AppState, Settings, create_router};

const TEST_CHUNK_SIZE: usize = 512;
const TEST_CHUNK_OVERLAP: usize = 50;
const TEST_TOP_K: usize = 5;
const TEST_SIMILARITY_THRESHOLD: f32 = 0.7;
const TEST_MAX_CONTEXT_TOKENS: usize = 3072;
const TEST_FALLBACK_MESSAGE: &str = "I cannot answer this based on the available lecture notes.";

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

struct MockLlmClient;

#[async_trait::async_trait]
impl LlmClient for MockLlmClient {
    async fn complete(&self, _prompt: &str, _context: &str) -> Result<String, LlmClientError> {
        Ok("Mock answer".to_string())
    }

    async fn complete_stream(
        &self,
        _prompt: &str,
        _context: &str,
    ) -> Result<
        std::pin::Pin<
            Box<
                dyn futures::stream::Stream<Item = Result<String, LlmClientError>> + Send + 'static,
            >,
        >,
        LlmClientError,
    > {
        Ok(Box::pin(futures::stream::once(async {
            Ok("Mock answer".to_string())
        })))
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
            chunk: Chunk::new("test chunk".to_string(), DocumentId::new(), Some(1), 0),
            score: 0.95,
        }])
    }

    async fn delete(&self, _chunk_ids: &[ChunkId]) -> Result<(), VectorStoreError> {
        Ok(())
    }
}

struct MockVectorStoreLowScore;

#[async_trait::async_trait]
impl VectorStore for MockVectorStoreLowScore {
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
            chunk: Chunk::new("test chunk".to_string(), DocumentId::new(), Some(1), 0),
            score: 0.3,
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

struct MockStagingStore;

#[async_trait::async_trait]
impl StagingStore for MockStagingStore {
    async fn store(
        &self,
        _path: &sandakan::domain::StoragePath,
        _stream: futures::stream::BoxStream<'_, Result<bytes::Bytes, std::io::Error>>,
        _content_length: Option<u64>,
    ) -> Result<u64, StagingStoreError> {
        Ok(0)
    }

    async fn fetch(
        &self,
        _path: &sandakan::domain::StoragePath,
    ) -> Result<Vec<u8>, StagingStoreError> {
        Ok(vec![])
    }

    async fn delete(&self, _path: &sandakan::domain::StoragePath) -> Result<(), StagingStoreError> {
        Ok(())
    }

    async fn head(&self, _path: &sandakan::domain::StoragePath) -> Result<u64, StagingStoreError> {
        Ok(0)
    }
}

fn mock_staging_store() -> Arc<dyn StagingStore> {
    Arc::new(MockStagingStore)
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
            similarity_threshold: TEST_SIMILARITY_THRESHOLD,
            max_context_tokens: TEST_MAX_CONTEXT_TOKENS,
            top_k: TEST_TOP_K,
            system_prompt: "test prompt".to_string(),
            fallback_message: TEST_FALLBACK_MESSAGE.to_string(),
        },
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

fn mock_job_repository() -> Arc<dyn JobRepository> {
    Arc::new(MockJobRepository)
}

fn mock_conversation_repository() -> Arc<dyn ConversationRepository> {
    Arc::new(MockConversationRepository)
}

fn create_ingestion_sender() -> tokio::sync::mpsc::Sender<IngestionMessage> {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(16);
    tokio::spawn(async move {
        while receiver.recv().await.is_some() {}
    });
    sender
}

fn create_test_app() -> axum::Router {
    use sandakan::infrastructure::text_processing::RecursiveCharacterSplitter;

    let file_loader = Arc::new(MockFileLoader);
    let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder);
    let llm_client = Arc::new(MockLlmClient);
    let vector_store = Arc::new(MockVectorStore);
    let text_splitter = Arc::new(RecursiveCharacterSplitter::new(
        TEST_CHUNK_SIZE,
        TEST_CHUNK_OVERLAP,
    ));

    let ingestion_service = Arc::new(IngestionService::new(
        Arc::clone(&file_loader),
        Arc::clone(&embedder),
        Arc::clone(&vector_store),
        text_splitter,
        mock_job_repository(),
    ));

    let retrieval_service = Arc::new(RetrievalService::new(
        Arc::clone(&embedder),
        Arc::clone(&llm_client),
        Arc::clone(&vector_store),
        mock_conversation_repository(),
        TEST_TOP_K,
        TEST_SIMILARITY_THRESHOLD,
        TEST_MAX_CONTEXT_TOKENS,
        TEST_FALLBACK_MESSAGE.to_string(),
    ));

    let state = AppState {
        ingestion_service,
        retrieval_service,
        conversation_repository: Arc::new(MockConversationRepository),
        job_repository: mock_job_repository(),
        ingestion_sender: create_ingestion_sender(),
        staging_store: mock_staging_store(),
        settings: test_settings(),
    };

    create_router(state)
}

#[tokio::test]
async fn given_running_server_when_health_check_then_returns_ok() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn given_valid_question_when_query_endpoint_then_returns_answer() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/query")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"question": "What is RAG?"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn given_missing_body_when_query_endpoint_then_returns_bad_request() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/query")
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn given_openwebui_when_requesting_models_then_returns_model_list() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn given_openwebui_when_requesting_api_models_then_returns_model_list() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn given_valid_chat_request_when_chat_completions_then_returns_response() {
    let app = create_test_app();

    let request_body = r#"{
        "model": "rag-pipeline",
        "messages": [
            {"role": "user", "content": "What is RAG?"}
        ]
    }"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn given_empty_messages_when_chat_completions_then_returns_bad_request() {
    let app = create_test_app();

    let request_body = r#"{
        "model": "rag-pipeline",
        "messages": []
    }"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn given_request_without_id_when_any_endpoint_then_response_contains_request_id() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response.headers().contains_key("x-request-id"));
}

#[tokio::test]
async fn given_request_with_id_when_any_endpoint_then_response_echoes_request_id() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("x-request-id", "test-request-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.headers().get("x-request-id").unwrap(),
        "test-request-123"
    );
}

#[tokio::test]
async fn given_low_similarity_when_chat_completions_then_returns_fallback() {
    use sandakan::infrastructure::text_processing::RecursiveCharacterSplitter;

    let file_loader = Arc::new(MockFileLoader);
    let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder);
    let llm_client = Arc::new(MockLlmClient);
    let vector_store = Arc::new(MockVectorStoreLowScore);
    let text_splitter = Arc::new(RecursiveCharacterSplitter::new(
        TEST_CHUNK_SIZE,
        TEST_CHUNK_OVERLAP,
    ));

    let ingestion_service = Arc::new(IngestionService::new(
        Arc::clone(&file_loader),
        Arc::clone(&embedder),
        Arc::clone(&vector_store),
        text_splitter,
        mock_job_repository(),
    ));

    let retrieval_service = Arc::new(RetrievalService::new(
        Arc::clone(&embedder),
        Arc::clone(&llm_client),
        Arc::clone(&vector_store),
        mock_conversation_repository(),
        TEST_TOP_K,
        TEST_SIMILARITY_THRESHOLD,
        TEST_MAX_CONTEXT_TOKENS,
        TEST_FALLBACK_MESSAGE.to_string(),
    ));

    let state = AppState {
        ingestion_service,
        retrieval_service,
        conversation_repository: Arc::new(MockConversationRepository),
        job_repository: mock_job_repository(),
        ingestion_sender: create_ingestion_sender(),
        staging_store: mock_staging_store(),
        settings: test_settings(),
    };

    let app = create_router(state);

    let request_body = r#"{
        "model": "rag-pipeline",
        "messages": [
            {"role": "user", "content": "What is RAG?"}
        ]
    }"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let content = json["choices"][0]["message"]["content"].as_str().unwrap();
    assert_eq!(content, TEST_FALLBACK_MESSAGE);
}

#[tokio::test]
async fn given_invalid_uuid_when_job_status_then_returns_bad_request() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/jobs/not-a-uuid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn given_nonexistent_job_when_job_status_then_returns_not_found() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/jobs/00000000-0000-0000-0000-000000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn given_valid_reference_when_ingesting_then_returns_accepted() {
    let app = create_test_app();

    let body = r#"{"storage_path":"some/path/video.mp4","filename":"video.mp4","content_type":"video/mp4"}"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/ingest-reference")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn given_unsupported_content_type_when_ingesting_reference_then_returns_415() {
    let app = create_test_app();

    let body = r#"{"storage_path":"some/path/file.xyz","filename":"file.xyz","content_type":"application/octet-stream"}"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/ingest-reference")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}
