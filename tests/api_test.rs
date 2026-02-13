mod application;
mod domain;
mod infrastructure;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use sandakan::application::ports::{
    CollectionConfig, ConversationRepository, Embedder, EmbedderError, FileLoader, FileLoaderError,
    JobRepository, LlmClient, LlmClientError, RepositoryError, SearchResult, VectorStore,
    VectorStoreError,
};
use sandakan::application::services::{IngestionService, RetrievalService};
use sandakan::domain::{
    Chunk, ChunkId, Conversation, ConversationId, Document, DocumentId, Embedding, Job, JobId,
    JobStatus, Message,
};
use sandakan::presentation::{AppState, ScaffoldConfig, create_router};

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

fn mock_job_repository() -> Arc<dyn JobRepository> {
    Arc::new(MockJobRepository)
}

fn mock_conversation_repository() -> Arc<dyn ConversationRepository> {
    Arc::new(MockConversationRepository)
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
        scaffold_config: ScaffoldConfig {
            enabled: false,
            mock_response_delay_ms: 0,
        },
    };

    create_router(state)
}

fn create_scaffold_app() -> axum::Router {
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
        scaffold_config: ScaffoldConfig {
            enabled: true,
            mock_response_delay_ms: 0,
        },
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
async fn given_scaffold_mode_when_chat_completions_then_echoes_message() {
    let app = create_scaffold_app();

    let request_body = r#"{
        "model": "rag-pipeline",
        "messages": [
            {"role": "user", "content": "Test Connection"}
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
    assert!(content.contains("Echo: Test Connection"));
}

#[tokio::test]
async fn given_scaffold_mode_with_stream_when_chat_completions_then_returns_sse() {
    let app = create_scaffold_app();

    let request_body = r#"{
        "model": "rag-pipeline",
        "messages": [
            {"role": "user", "content": "Hello"}
        ],
        "stream": true
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
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
}

#[tokio::test]
async fn given_scaffold_mode_with_empty_message_when_chat_then_returns_bad_request() {
    let app = create_scaffold_app();

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
        scaffold_config: ScaffoldConfig {
            enabled: false,
            mock_response_delay_ms: 0,
        },
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
