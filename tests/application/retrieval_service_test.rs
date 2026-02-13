use std::sync::Arc;

use sandakan::application::ports::{
    CollectionConfig, ConversationRepository, Embedder, EmbedderError, LlmClient, LlmClientError,
    RepositoryError, SearchResult, VectorStore, VectorStoreError,
};
use sandakan::application::services::RetrievalService;
use sandakan::domain::{
    Chunk, ChunkId, Conversation, ConversationId, DocumentId, Embedding, Message,
};

const TEST_TOP_K: usize = 5;
const TEST_SIMILARITY_THRESHOLD: f32 = 0.7;
const TEST_MAX_CONTEXT_TOKENS: usize = 3072;
const TEST_FALLBACK_MESSAGE: &str = "I cannot answer this based on the available lecture notes.";

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

struct MockVectorStoreHighScore;

#[async_trait::async_trait]
impl VectorStore for MockVectorStoreHighScore {
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

struct MockVectorStoreEmpty;

#[async_trait::async_trait]
impl VectorStore for MockVectorStoreEmpty {
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
        Ok(vec![])
    }

    async fn delete(&self, _chunk_ids: &[ChunkId]) -> Result<(), VectorStoreError> {
        Ok(())
    }
}

struct MockVectorStoreManyChunks;

#[async_trait::async_trait]
impl VectorStore for MockVectorStoreManyChunks {
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
        let long_text = "This is a very long chunk that contains many tokens to test the context window management. ".repeat(100);
        let mut results = Vec::new();
        for i in 0..50 {
            results.push(SearchResult {
                chunk: Chunk::new(long_text.clone(), DocumentId::new(), Some(i), i as usize),
                score: 0.9 - (i as f32 * 0.001),
            });
        }
        Ok(results)
    }

    async fn delete(&self, _chunk_ids: &[ChunkId]) -> Result<(), VectorStoreError> {
        Ok(())
    }
}

struct MockVectorStoreBoundaryScore;

#[async_trait::async_trait]
impl VectorStore for MockVectorStoreBoundaryScore {
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
            chunk: Chunk::new("boundary test".to_string(), DocumentId::new(), Some(1), 0),
            score: 0.7,
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

fn mock_conversation_repository() -> Arc<dyn ConversationRepository> {
    Arc::new(MockConversationRepository)
}

#[tokio::test]
async fn given_high_similarity_results_when_querying_then_returns_llm_answer() {
    let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder);
    let llm_client = Arc::new(MockLlmClient);
    let vector_store = Arc::new(MockVectorStoreHighScore);

    let service = RetrievalService::new(
        embedder,
        llm_client,
        vector_store,
        mock_conversation_repository(),
        TEST_TOP_K,
        TEST_SIMILARITY_THRESHOLD,
        TEST_MAX_CONTEXT_TOKENS,
        TEST_FALLBACK_MESSAGE.to_string(),
    );

    let result = service.query("test question", None).await.unwrap();

    assert_eq!(result.answer, "Mock answer");
    assert!(!result.sources.is_empty());
}

#[tokio::test]
async fn given_low_similarity_results_when_querying_then_returns_fallback() {
    let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder);
    let llm_client = Arc::new(MockLlmClient);
    let vector_store = Arc::new(MockVectorStoreLowScore);

    let service = RetrievalService::new(
        embedder,
        llm_client,
        vector_store,
        mock_conversation_repository(),
        TEST_TOP_K,
        TEST_SIMILARITY_THRESHOLD,
        TEST_MAX_CONTEXT_TOKENS,
        TEST_FALLBACK_MESSAGE.to_string(),
    );

    let result = service.query("test question", None).await.unwrap();

    assert_eq!(result.answer, TEST_FALLBACK_MESSAGE);
    assert!(result.sources.is_empty());
}

#[tokio::test]
async fn given_no_search_results_when_querying_then_returns_fallback() {
    let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder);
    let llm_client = Arc::new(MockLlmClient);
    let vector_store = Arc::new(MockVectorStoreEmpty);

    let service = RetrievalService::new(
        embedder,
        llm_client,
        vector_store,
        mock_conversation_repository(),
        TEST_TOP_K,
        TEST_SIMILARITY_THRESHOLD,
        TEST_MAX_CONTEXT_TOKENS,
        TEST_FALLBACK_MESSAGE.to_string(),
    );

    let result = service.query("test question", None).await.unwrap();

    assert_eq!(result.answer, TEST_FALLBACK_MESSAGE);
    assert!(result.sources.is_empty());
}

#[tokio::test]
async fn given_many_chunks_exceeding_budget_when_querying_then_context_is_trimmed() {
    let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder);
    let llm_client = Arc::new(MockLlmClient);
    let vector_store = Arc::new(MockVectorStoreManyChunks);

    let service = RetrievalService::new(
        embedder,
        llm_client,
        vector_store,
        mock_conversation_repository(),
        TEST_TOP_K,
        TEST_SIMILARITY_THRESHOLD,
        TEST_MAX_CONTEXT_TOKENS,
        TEST_FALLBACK_MESSAGE.to_string(),
    );

    let result = service.query("test question", None).await.unwrap();

    assert_eq!(result.answer, "Mock answer");
    assert!(!result.sources.is_empty());
}

#[tokio::test]
async fn given_threshold_boundary_score_when_querying_then_includes_exact_match() {
    let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder);
    let llm_client = Arc::new(MockLlmClient);
    let vector_store = Arc::new(MockVectorStoreBoundaryScore);

    let service = RetrievalService::new(
        embedder,
        llm_client,
        vector_store,
        mock_conversation_repository(),
        TEST_TOP_K,
        TEST_SIMILARITY_THRESHOLD,
        TEST_MAX_CONTEXT_TOKENS,
        TEST_FALLBACK_MESSAGE.to_string(),
    );

    let result = service.query("test question", None).await.unwrap();

    assert_eq!(result.answer, "Mock answer");
    assert_eq!(result.sources.len(), 1);
    assert_eq!(result.sources[0].score, 0.7);
}
