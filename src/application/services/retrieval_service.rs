use std::sync::Arc;

use async_trait::async_trait;

use crate::application::ports::{
    ConversationRepository, Embedder, EvalEventRepository, EvalOutboxRepository, LlmClient,
    LlmTokenStream, RetrievalError, RetrievalServicePort, SourceChunk, VectorStore,
};
use crate::application::services::count_tokens;
use crate::domain::{ConversationId, EvalEvent, EvalSource, Message, MessageRole};

pub struct RetrievalService<L, V>
where
    L: LlmClient,
    V: VectorStore,
{
    embedder: Arc<dyn Embedder>,
    llm_client: Arc<L>,
    vector_store: Arc<V>,
    conversation_repository: Arc<dyn ConversationRepository>,
    eval_event_repository: Option<Arc<dyn EvalEventRepository>>,
    eval_outbox_repository: Option<Arc<dyn EvalOutboxRepository>>,
    model_config: String,
    top_k: usize,
    similarity_threshold: f32,
    max_context_tokens: usize,
    fallback_message: String,
}

impl<L, V> RetrievalService<L, V>
where
    L: LlmClient,
    V: VectorStore,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        embedder: Arc<dyn Embedder>,
        llm_client: Arc<L>,
        vector_store: Arc<V>,
        conversation_repository: Arc<dyn ConversationRepository>,
        eval_event_repository: Option<Arc<dyn EvalEventRepository>>,
        eval_outbox_repository: Option<Arc<dyn EvalOutboxRepository>>,
        model_config: String,
        top_k: usize,
        similarity_threshold: f32,
        max_context_tokens: usize,
        fallback_message: String,
    ) -> Self {
        Self {
            embedder,
            llm_client,
            vector_store,
            conversation_repository,
            eval_event_repository,
            eval_outbox_repository,
            model_config,
            top_k,
            similarity_threshold,
            max_context_tokens,
            fallback_message,
        }
    }

    #[tracing::instrument(
        skip(self, question, conversation_id),
        fields(retrieved_chunks_count, similarity_score)
    )]
    pub async fn query(
        &self,
        question: &str,
        conversation_id: Option<ConversationId>,
    ) -> Result<QueryResponse, RetrievalError> {
        let query_embedding = self
            .embedder
            .embed(question)
            .await
            .map_err(RetrievalError::Embedding)?;

        let results = self
            .vector_store
            .search(&query_embedding, self.top_k)
            .await
            .map_err(RetrievalError::Search)?;

        if results.is_empty()
            || results
                .first()
                .map(|r| r.score < self.similarity_threshold)
                .unwrap_or(false)
        {
            tracing::Span::current().record("retrieved_chunks_count", 0);
            tracing::Span::current().record(
                "similarity_score",
                results.first().map(|r| r.score).unwrap_or(0.0),
            );
            return Ok(QueryResponse {
                answer: self.fallback_message.clone(),
                sources: Vec::new(),
            });
        }

        let filtered_results: Vec<_> = results
            .into_iter()
            .filter(|r| r.score >= self.similarity_threshold)
            .collect();

        let mut accumulated_tokens = 0;
        let mut trimmed_chunks = Vec::new();

        for result in &filtered_results {
            let chunk_tokens = count_tokens(&result.chunk.text);
            if accumulated_tokens + chunk_tokens <= self.max_context_tokens {
                accumulated_tokens += chunk_tokens;
                trimmed_chunks.push(result);
            } else {
                break;
            }
        }

        tracing::Span::current().record("retrieved_chunks_count", trimmed_chunks.len());
        tracing::Span::current().record(
            "similarity_score",
            trimmed_chunks.first().map(|r| r.score).unwrap_or(0.0),
        );

        let context = trimmed_chunks
            .iter()
            .map(|r| r.chunk.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let answer = self
            .llm_client
            .complete(question, &context)
            .await
            .map_err(RetrievalError::Completion)?;

        if let Some(conv_id) = conversation_id {
            let user_message = Message::new(conv_id, MessageRole::User, question.to_string());
            self.conversation_repository
                .append_message(&user_message)
                .await
                .map_err(RetrievalError::Repository)?;

            let assistant_message = Message::new(conv_id, MessageRole::Assistant, answer.clone());
            self.conversation_repository
                .append_message(&assistant_message)
                .await
                .map_err(RetrievalError::Repository)?;
        }

        let sources: Vec<SourceChunk> = trimmed_chunks
            .into_iter()
            .map(|r| SourceChunk {
                text: r.chunk.text.clone(),
                page: r.chunk.page,
                score: r.score,
            })
            .collect();

        if let (Some(event_repo), Some(outbox_repo)) =
            (&self.eval_event_repository, &self.eval_outbox_repository)
        {
            let eval_sources: Vec<EvalSource> = sources
                .iter()
                .map(|s| EvalSource {
                    text: s.text.clone(),
                    page: s.page,
                    score: s.score,
                })
                .collect();
            let eval_event = EvalEvent::new(question, &answer, eval_sources, &self.model_config);
            let event_repo = Arc::clone(event_repo);
            let outbox_repo = Arc::clone(outbox_repo);
            tokio::spawn(async move {
                match event_repo.record(&eval_event).await {
                    Ok(_) => {
                        if let Err(e) = outbox_repo.enqueue(eval_event.id).await {
                            tracing::warn!(error = %e, "Failed to enqueue eval outbox");
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "Failed to record eval event"),
                }
            });
        }

        Ok(QueryResponse { answer, sources })
    }

    #[tracing::instrument(
        skip(self, question, conversation_id),
        fields(retrieved_chunks_count, similarity_score)
    )]
    pub async fn query_stream(
        &self,
        question: &str,
        conversation_id: Option<ConversationId>,
    ) -> Result<StreamingQueryResponse, RetrievalError> {
        let query_embedding = self
            .embedder
            .embed(question)
            .await
            .map_err(RetrievalError::Embedding)?;

        let results = self
            .vector_store
            .search(&query_embedding, self.top_k)
            .await
            .map_err(RetrievalError::Search)?;

        if results.is_empty()
            || results
                .first()
                .map(|r| r.score < self.similarity_threshold)
                .unwrap_or(false)
        {
            tracing::Span::current().record("retrieved_chunks_count", 0);
            tracing::Span::current().record(
                "similarity_score",
                results.first().map(|r| r.score).unwrap_or(0.0),
            );

            let fallback = self.fallback_message.clone();
            let token_stream = Box::pin(futures::stream::once(async move { Ok(fallback) }));
            return Ok(StreamingQueryResponse {
                token_stream,
                sources: Vec::new(),
                conversation_id,
            });
        }

        let filtered_results: Vec<_> = results
            .into_iter()
            .filter(|r| r.score >= self.similarity_threshold)
            .collect();

        let mut accumulated_tokens = 0;
        let mut trimmed_chunks = Vec::new();

        for result in &filtered_results {
            let chunk_tokens = count_tokens(&result.chunk.text);
            if accumulated_tokens + chunk_tokens <= self.max_context_tokens {
                accumulated_tokens += chunk_tokens;
                trimmed_chunks.push(result);
            } else {
                break;
            }
        }

        tracing::Span::current().record("retrieved_chunks_count", trimmed_chunks.len());
        tracing::Span::current().record(
            "similarity_score",
            trimmed_chunks.first().map(|r| r.score).unwrap_or(0.0),
        );

        let context = trimmed_chunks
            .iter()
            .map(|r| r.chunk.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let token_stream = self
            .llm_client
            .complete_stream(question, &context)
            .await
            .map_err(RetrievalError::Completion)?;

        let sources = trimmed_chunks
            .into_iter()
            .map(|r| SourceChunk {
                text: r.chunk.text.clone(),
                page: r.chunk.page,
                score: r.score,
            })
            .collect();

        Ok(StreamingQueryResponse {
            token_stream,
            sources,
            conversation_id,
        })
    }

    pub async fn search_chunks(&self, query: &str) -> Result<Vec<SourceChunk>, RetrievalError> {
        let query_embedding = self
            .embedder
            .embed(query)
            .await
            .map_err(RetrievalError::Embedding)?;

        let results = self
            .vector_store
            .search(&query_embedding, self.top_k)
            .await
            .map_err(RetrievalError::Search)?;

        if results.is_empty()
            || results
                .first()
                .map(|r| r.score < self.similarity_threshold)
                .unwrap_or(false)
        {
            return Ok(Vec::new());
        }

        let filtered_results: Vec<_> = results
            .into_iter()
            .filter(|r| r.score >= self.similarity_threshold)
            .collect();

        let mut accumulated_tokens = 0;
        let mut trimmed_chunks = Vec::new();

        for result in &filtered_results {
            let chunk_tokens = count_tokens(&result.chunk.text);
            if accumulated_tokens + chunk_tokens <= self.max_context_tokens {
                accumulated_tokens += chunk_tokens;
                trimmed_chunks.push(result);
            } else {
                break;
            }
        }

        let chunks = trimmed_chunks
            .into_iter()
            .map(|r| SourceChunk {
                text: r.chunk.text.clone(),
                page: r.chunk.page,
                score: r.score,
            })
            .collect();

        Ok(chunks)
    }
}

#[async_trait]
impl<L, V> RetrievalServicePort for RetrievalService<L, V>
where
    L: LlmClient,
    V: VectorStore,
{
    async fn search_chunks(&self, query: &str) -> Result<Vec<SourceChunk>, RetrievalError> {
        self.search_chunks(query).await
    }
}

#[derive(Debug, Clone)]
pub struct QueryResponse {
    pub answer: String,
    pub sources: Vec<SourceChunk>,
}

pub struct StreamingQueryResponse {
    pub token_stream: LlmTokenStream,
    pub sources: Vec<SourceChunk>,
    pub conversation_id: Option<ConversationId>,
}
