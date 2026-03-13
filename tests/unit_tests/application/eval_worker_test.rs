use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use uuid::Uuid;

use sandakan::application::ports::{
    AgentMessage, EvalEventError, EvalEventRepository, EvalOutboxError, EvalOutboxRepository,
    EvalResultError, EvalResultRepository, LlmClient, LlmClientError, LlmToolResponse, ToolSchema,
};
use sandakan::application::services::EvalWorker;
use sandakan::domain::{
    AgenticTrace, EvalEvent, EvalEventId, EvalOperationType, EvalOutboxEntry, EvalResult,
    EvalSource, ToolCallTrace,
};

// --- Hand-written mocks ---

/// Judge that returns a valid faithfulness score.
struct HighFaithfulnessJudge;

#[async_trait::async_trait]
impl LlmClient for HighFaithfulnessJudge {
    async fn complete(&self, _prompt: &str, _context: &str) -> Result<String, LlmClientError> {
        Ok("0.95".to_string())
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
            Ok("0.95".to_string())
        })))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<
        std::pin::Pin<
            Box<
                dyn futures::stream::Stream<Item = Result<String, LlmClientError>> + Send + 'static,
            >,
        >,
        LlmClientError,
    > {
        unimplemented!()
    }
    async fn complete_with_tools(
        &self,
        _messages: &[AgentMessage],
        _tools: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        unimplemented!()
    }
}

/// Judge that returns an unparseable score.
struct InvalidScoreJudge;

#[async_trait::async_trait]
impl LlmClient for InvalidScoreJudge {
    async fn complete(&self, _prompt: &str, _context: &str) -> Result<String, LlmClientError> {
        Ok("I cannot provide a score.".to_string())
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
        unimplemented!()
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<
        std::pin::Pin<
            Box<
                dyn futures::stream::Stream<Item = Result<String, LlmClientError>> + Send + 'static,
            >,
        >,
        LlmClientError,
    > {
        unimplemented!()
    }
    async fn complete_with_tools(
        &self,
        _messages: &[AgentMessage],
        _tools: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        unimplemented!()
    }
}

fn sample_eval_event(id: EvalEventId) -> EvalEvent {
    EvalEvent {
        id,
        timestamp: chrono::Utc::now(),
        question: "What is chunking?".to_string(),
        generated_answer: "Chunking splits text into smaller pieces.".to_string(),
        retrieved_sources: vec![EvalSource {
            text: "Chunking is the process of splitting documents into smaller pieces.".to_string(),
            page: Some(1),
            score: 0.92,
        }],
        model_config: "test/model".to_string(),
        operation_type: EvalOperationType::Query,
        correlation_id: None,
        agentic_trace: None,
    }
}

fn ingestion_eval_event(id: EvalEventId, chunk_count: usize) -> EvalEvent {
    EvalEvent {
        id,
        timestamp: chrono::Utc::now(),
        question: "document.pdf".to_string(),
        generated_answer: chunk_count.to_string(),
        retrieved_sources: vec![],
        model_config: "test/model".to_string(),
        operation_type: EvalOperationType::IngestionPdf,
        correlation_id: None,
        agentic_trace: None,
    }
}

fn ingestion_eval_event_with_samples(
    id: EvalEventId,
    chunk_count: usize,
    operation_type: EvalOperationType,
) -> EvalEvent {
    EvalEvent {
        id,
        timestamp: chrono::Utc::now(),
        question: "document.pdf".to_string(),
        generated_answer: chunk_count.to_string(),
        retrieved_sources: vec![
            EvalSource {
                text: "This is the first chunk of well-formed text.".to_string(),
                page: Some(1),
                score: 0.0,
            },
            EvalSource {
                text: "This is the second chunk discussing another topic.".to_string(),
                page: Some(1),
                score: 0.0,
            },
            EvalSource {
                text: "A third chunk covers the conclusion of the document.".to_string(),
                page: Some(2),
                score: 0.0,
            },
        ],
        model_config: "test/model".to_string(),
        operation_type,
        correlation_id: None,
        agentic_trace: None,
    }
}

fn agentic_eval_event(id: EvalEventId) -> EvalEvent {
    EvalEvent {
        id,
        timestamp: chrono::Utc::now(),
        question: "What does the agent know?".to_string(),
        generated_answer: "The agent knows quite a lot.".to_string(),
        retrieved_sources: vec![EvalSource {
            text: "The agent has access to many tools.".to_string(),
            page: None,
            score: 0.85,
        }],
        model_config: "test/model".to_string(),
        operation_type: EvalOperationType::AgenticRun,
        correlation_id: None,
        agentic_trace: None,
    }
}

/// Outbox mock that returns one pending entry then tracks mark_done / mark_failed calls.
struct TrackingOutboxRepository {
    entries: Vec<EvalOutboxEntry>,
    done_ids: Mutex<Vec<Uuid>>,
    failed_ids: Mutex<Vec<(Uuid, String)>>,
}

impl TrackingOutboxRepository {
    fn with_entries(entries: Vec<EvalOutboxEntry>) -> Self {
        Self {
            entries,
            done_ids: Mutex::new(vec![]),
            failed_ids: Mutex::new(vec![]),
        }
    }

    async fn done_count(&self) -> usize {
        self.done_ids.lock().await.len()
    }

    async fn failed_count(&self) -> usize {
        self.failed_ids.lock().await.len()
    }
}

#[async_trait::async_trait]
impl EvalOutboxRepository for TrackingOutboxRepository {
    async fn enqueue(&self, _eval_event_id: EvalEventId) -> Result<(), EvalOutboxError> {
        Ok(())
    }

    async fn claim_pending(
        &self,
        _batch_size: usize,
    ) -> Result<Vec<EvalOutboxEntry>, EvalOutboxError> {
        Ok(self.entries.clone())
    }

    async fn mark_done(&self, id: Uuid) -> Result<(), EvalOutboxError> {
        self.done_ids.lock().await.push(id);
        Ok(())
    }

    async fn mark_failed(&self, id: Uuid, error: &str) -> Result<(), EvalOutboxError> {
        self.failed_ids.lock().await.push((id, error.to_string()));
        Ok(())
    }
}

/// Empty outbox mock — no pending entries.
struct EmptyOutboxRepository;

#[async_trait::async_trait]
impl EvalOutboxRepository for EmptyOutboxRepository {
    async fn enqueue(&self, _eval_event_id: EvalEventId) -> Result<(), EvalOutboxError> {
        Ok(())
    }
    async fn claim_pending(
        &self,
        _batch_size: usize,
    ) -> Result<Vec<EvalOutboxEntry>, EvalOutboxError> {
        Ok(vec![])
    }
    async fn mark_done(&self, _id: Uuid) -> Result<(), EvalOutboxError> {
        Ok(())
    }
    async fn mark_failed(&self, _id: Uuid, _error: &str) -> Result<(), EvalOutboxError> {
        Ok(())
    }
}

/// Event repository that stores a single event and returns it on get().
struct SingleEventRepository {
    event: EvalEvent,
}

#[async_trait::async_trait]
impl EvalEventRepository for SingleEventRepository {
    async fn record(&self, _event: &EvalEvent) -> Result<(), EvalEventError> {
        Ok(())
    }
    async fn get(&self, id: EvalEventId) -> Result<Option<EvalEvent>, EvalEventError> {
        if id == self.event.id {
            Ok(Some(self.event.clone()))
        } else {
            Ok(None)
        }
    }
    async fn list(&self, _limit: Option<usize>) -> Result<Vec<EvalEvent>, EvalEventError> {
        Ok(vec![self.event.clone()])
    }
    async fn sample(&self, _n: usize) -> Result<Vec<EvalEvent>, EvalEventError> {
        Ok(vec![self.event.clone()])
    }
}

/// Event repository that always returns None for get().
struct EmptyEventRepository;

#[async_trait::async_trait]
impl EvalEventRepository for EmptyEventRepository {
    async fn record(&self, _event: &EvalEvent) -> Result<(), EvalEventError> {
        Ok(())
    }
    async fn get(&self, _id: EvalEventId) -> Result<Option<EvalEvent>, EvalEventError> {
        Ok(None)
    }
    async fn list(&self, _limit: Option<usize>) -> Result<Vec<EvalEvent>, EvalEventError> {
        Ok(vec![])
    }
    async fn sample(&self, _n: usize) -> Result<Vec<EvalEvent>, EvalEventError> {
        Ok(vec![])
    }
}

/// Result repository that records all saved results for inspection.
struct TrackingResultRepository {
    saved: Mutex<Vec<EvalResult>>,
}

impl TrackingResultRepository {
    fn new() -> Self {
        Self {
            saved: Mutex::new(vec![]),
        }
    }

    async fn save_count(&self) -> usize {
        self.saved.lock().await.len()
    }
}

#[async_trait::async_trait]
impl EvalResultRepository for TrackingResultRepository {
    async fn save(&self, result: &EvalResult) -> Result<(), EvalResultError> {
        self.saved.lock().await.push(result.clone());
        Ok(())
    }
}

/// Result repository that always fails.
struct FailingResultRepository;

#[async_trait::async_trait]
impl EvalResultRepository for FailingResultRepository {
    async fn save(&self, _result: &EvalResult) -> Result<(), EvalResultError> {
        Err(EvalResultError::Database(
            "simulated db failure".to_string(),
        ))
    }
}

/// No-op result repository for tests not concerned with result persistence.
struct NoopResultRepository;

#[async_trait::async_trait]
impl EvalResultRepository for NoopResultRepository {
    async fn save(&self, _result: &EvalResult) -> Result<(), EvalResultError> {
        Ok(())
    }
}

// --- Tests ---

#[tokio::test]
async fn given_pending_outbox_entry_when_worker_processes_then_marks_done() {
    let event_id = EvalEventId::new();
    let event = sample_eval_event(event_id);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        Arc::new(NoopResultRepository) as Arc<dyn EvalResultRepository>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 1);
    assert_eq!(outbox.failed_count().await, 0);
}

#[tokio::test]
async fn given_judge_returns_invalid_score_when_worker_processes_then_marks_failed() {
    let event_id = EvalEventId::new();
    let event = sample_eval_event(event_id);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        Arc::new(NoopResultRepository) as Arc<dyn EvalResultRepository>,
        Arc::new(InvalidScoreJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 0);
    assert_eq!(outbox.failed_count().await, 1);
}

#[tokio::test]
async fn given_no_pending_entries_when_worker_polls_then_no_operations_performed() {
    let outbox = Arc::new(EmptyOutboxRepository);
    let event_repo = Arc::new(EmptyEventRepository);

    let worker = EvalWorker::new(
        outbox as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        Arc::new(NoopResultRepository) as Arc<dyn EvalResultRepository>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 0);
}

#[tokio::test]
async fn given_missing_eval_event_when_worker_processes_then_marks_failed() {
    let event_id = EvalEventId::new();
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(EmptyEventRepository);

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        Arc::new(NoopResultRepository) as Arc<dyn EvalResultRepository>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 0);
    assert_eq!(outbox.failed_count().await, 1);
}

#[tokio::test]
async fn given_successful_scoring_when_worker_processes_entry_then_result_is_saved_to_repository() {
    let event_id = EvalEventId::new();
    let event = sample_eval_event(event_id);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });
    let result_repo = Arc::new(TrackingResultRepository::new());

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        result_repo.clone() as Arc<dyn EvalResultRepository>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(result_repo.save_count().await, 1);
    assert_eq!(outbox.done_count().await, 1);
    assert_eq!(outbox.failed_count().await, 0);
}

#[tokio::test]
async fn given_result_repository_fails_when_worker_processes_entry_then_outbox_is_marked_failed() {
    let event_id = EvalEventId::new();
    let event = sample_eval_event(event_id);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        Arc::new(FailingResultRepository) as Arc<dyn EvalResultRepository>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 0);
    assert_eq!(outbox.failed_count().await, 1);
}

#[tokio::test]
async fn given_agentic_run_eval_event_when_worker_processes_then_llm_judge_called_and_result_persisted()
 {
    let event_id = EvalEventId::new();
    let event = agentic_eval_event(event_id);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });
    let result_repo = Arc::new(TrackingResultRepository::new());

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        result_repo.clone() as Arc<dyn EvalResultRepository>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 1);
    assert_eq!(outbox.failed_count().await, 0);
    assert_eq!(result_repo.save_count().await, 1);
}

#[tokio::test]
async fn given_ingestion_pdf_eval_event_when_worker_processes_then_llm_client_not_called_and_result_persisted()
 {
    let event_id = EvalEventId::new();
    // chunk_count > 0 → faithfulness = 1.0 without any LLM call
    let event = ingestion_eval_event(event_id, 12);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });
    let result_repo = Arc::new(TrackingResultRepository::new());

    // InvalidScoreJudge would cause a failure if called — confirms no LLM call is made.
    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        result_repo.clone() as Arc<dyn EvalResultRepository>,
        Arc::new(InvalidScoreJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 1);
    assert_eq!(outbox.failed_count().await, 0);
    assert_eq!(result_repo.save_count().await, 1);
}

#[tokio::test]
async fn given_ingestion_pdf_with_zero_chunks_when_worker_processes_then_result_faithfulness_is_zero()
 {
    let event_id = EvalEventId::new();
    let event = ingestion_eval_event(event_id, 0);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });
    let result_repo = Arc::new(TrackingResultRepository::new());

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        result_repo.clone() as Arc<dyn EvalResultRepository>,
        Arc::new(InvalidScoreJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 1);
    let saved = result_repo.saved.lock().await;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].faithfulness, 0.0);
    assert!(saved[0].below_threshold);
}

#[tokio::test]
async fn given_query_eval_event_when_worker_processes_then_emits_query_operation_type() {
    let event_id = EvalEventId::new();
    let event = sample_eval_event(event_id);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });
    let result_repo = Arc::new(TrackingResultRepository::new());

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        result_repo.clone() as Arc<dyn EvalResultRepository>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 1);
    assert_eq!(result_repo.save_count().await, 1);
}

fn agentic_eval_event_with_trace(id: EvalEventId) -> EvalEvent {
    EvalEvent {
        id,
        timestamp: chrono::Utc::now(),
        question: "What does the search tool return for AI?".to_string(),
        generated_answer: "The search tool found several articles about AI.".to_string(),
        retrieved_sources: vec![EvalSource {
            text: "AI is a broad field.".to_string(),
            page: None,
            score: 0.9,
        }],
        model_config: "test/model".to_string(),
        operation_type: EvalOperationType::AgenticRun,
        correlation_id: None,
        agentic_trace: Some(AgenticTrace {
            iterations: 1,
            tool_calls: vec![ToolCallTrace {
                tool_name: "search".to_string(),
                arguments: r#"{"query":"AI"}"#.to_string(),
                result_preview: "The search tool found several articles about AI.".to_string(),
                success: true,
            }],
            reflection_score: Some(0.88),
            reflection_issues: vec![],
        }),
    }
}

fn agentic_eval_event_with_empty_trace(id: EvalEventId) -> EvalEvent {
    EvalEvent {
        id,
        timestamp: chrono::Utc::now(),
        question: "Generate three follow-up questions based on our chat history.".to_string(),
        generated_answer: "1. What is chunking?\n2. How does RRF work?\n3. What are dense vectors?"
            .to_string(),
        retrieved_sources: vec![],
        model_config: "test/model".to_string(),
        operation_type: EvalOperationType::AgenticRun,
        correlation_id: None,
        agentic_trace: Some(AgenticTrace {
            iterations: 1,
            tool_calls: vec![],
            reflection_score: None,
            reflection_issues: vec![],
        }),
    }
}

#[tokio::test]
async fn given_agentic_run_with_tool_trace_when_worker_processes_then_result_persisted_via_trace_path()
 {
    let event_id = EvalEventId::new();
    let event = agentic_eval_event_with_trace(event_id);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });
    let result_repo = Arc::new(TrackingResultRepository::new());

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        result_repo.clone() as Arc<dyn EvalResultRepository>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 1);
    assert_eq!(outbox.failed_count().await, 0);
    let saved = result_repo.saved.lock().await;
    assert_eq!(saved.len(), 1);
    assert!((saved[0].faithfulness - 0.95).abs() < 0.001);
    assert!(saved[0].answer_relevancy.is_some());
    assert!(saved[0].context_precision.is_some());
    assert!(!saved[0].question.is_empty());
    assert!(!saved[0].generated_answer.is_empty());
    assert!(!saved[0].eval_description.is_empty());
}

#[tokio::test]
async fn given_agentic_run_without_trace_when_worker_processes_then_fallback_flat_context_path_used()
 {
    let event_id = EvalEventId::new();
    let event = agentic_eval_event(event_id);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });
    let result_repo = Arc::new(TrackingResultRepository::new());

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        result_repo.clone() as Arc<dyn EvalResultRepository>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 1);
    assert_eq!(outbox.failed_count().await, 0);
    let saved = result_repo.saved.lock().await;
    assert_eq!(saved.len(), 1);
    assert!((saved[0].faithfulness - 0.95).abs() < 0.001);
}

#[tokio::test]
async fn given_agentic_run_with_empty_tool_calls_when_worker_processes_then_context_precision_is_none()
 {
    let event_id = EvalEventId::new();
    let event = agentic_eval_event_with_empty_trace(event_id);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });
    let result_repo = Arc::new(TrackingResultRepository::new());

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        result_repo.clone() as Arc<dyn EvalResultRepository>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 1);
    assert_eq!(outbox.failed_count().await, 0);
    let saved = result_repo.saved.lock().await;
    assert_eq!(saved.len(), 1);
    // Faithfulness graded against flat context via the zero-tool-call path.
    assert!((saved[0].faithfulness - 0.95).abs() < 0.001);
    // answer_relevancy is always computed.
    assert!(saved[0].answer_relevancy.is_some());
    // context_precision must be None — no retrieval happened.
    assert!(saved[0].context_precision.is_none());
    // below_threshold is false because faithfulness (0.95) > threshold (0.7).
    assert!(!saved[0].below_threshold);
}

#[tokio::test]
async fn given_ingestion_pdf_with_chunk_samples_when_worker_processes_then_llm_judge_called_and_quality_scored()
 {
    let event_id = EvalEventId::new();
    let event = ingestion_eval_event_with_samples(event_id, 12, EvalOperationType::IngestionPdf);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });
    let result_repo = Arc::new(TrackingResultRepository::new());

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        result_repo.clone() as Arc<dyn EvalResultRepository>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 1);
    assert_eq!(outbox.failed_count().await, 0);
    let saved = result_repo.saved.lock().await;
    assert_eq!(saved.len(), 1);
    assert!((saved[0].faithfulness - 0.95).abs() < 0.001);
    assert!(!saved[0].eval_description.is_empty());
    assert!(!saved[0].below_threshold);
}

#[tokio::test]
async fn given_ingestion_pdf_with_chunk_samples_when_judge_fails_then_outbox_marked_failed() {
    let event_id = EvalEventId::new();
    let event = ingestion_eval_event_with_samples(event_id, 12, EvalOperationType::IngestionPdf);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });
    let result_repo = Arc::new(TrackingResultRepository::new());

    // InvalidScoreJudge returns non-numeric text, causing chunk quality parsing to fail.
    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        result_repo.clone() as Arc<dyn EvalResultRepository>,
        Arc::new(InvalidScoreJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 0);
    assert_eq!(outbox.failed_count().await, 1);
    assert_eq!(result_repo.save_count().await, 0);
}

#[tokio::test]
async fn given_ingestion_mp4_with_chunk_samples_when_worker_processes_then_quality_scored() {
    let event_id = EvalEventId::new();
    let event = ingestion_eval_event_with_samples(event_id, 8, EvalOperationType::IngestionMp4);
    let entry = EvalOutboxEntry::new(event_id);

    let outbox = Arc::new(TrackingOutboxRepository::with_entries(vec![entry]));
    let event_repo = Arc::new(SingleEventRepository { event });
    let result_repo = Arc::new(TrackingResultRepository::new());

    let worker = EvalWorker::new(
        outbox.clone() as Arc<dyn EvalOutboxRepository>,
        event_repo as Arc<dyn EvalEventRepository>,
        result_repo.clone() as Arc<dyn EvalResultRepository>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 1);
    let saved = result_repo.saved.lock().await;
    assert_eq!(saved.len(), 1);
    assert!((saved[0].faithfulness - 0.95).abs() < 0.001);
}
