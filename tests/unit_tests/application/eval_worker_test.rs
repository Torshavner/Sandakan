use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use uuid::Uuid;

use sandakan::application::ports::{
    Embedder, EmbedderError, EvalEventError, EvalEventRepository, EvalOutboxError,
    EvalOutboxRepository, EvalResultError, EvalResultRepository, LlmClient, LlmClientError,
};
use sandakan::application::services::EvalWorker;
use sandakan::domain::{
    Embedding, EvalEvent, EvalEventId, EvalOutboxEntry, EvalResult, EvalSource,
};

// --- Hand-written mocks ---

struct StubEmbedder;

#[async_trait::async_trait]
impl Embedder for StubEmbedder {
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
        Arc::new(StubEmbedder) as Arc<dyn Embedder>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
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
        Arc::new(StubEmbedder) as Arc<dyn Embedder>,
        Arc::new(InvalidScoreJudge) as Arc<dyn LlmClient>,
        0.7,
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
        Arc::new(StubEmbedder) as Arc<dyn Embedder>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
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
        Arc::new(StubEmbedder) as Arc<dyn Embedder>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
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
        Arc::new(StubEmbedder) as Arc<dyn Embedder>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
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
        Arc::new(StubEmbedder) as Arc<dyn Embedder>,
        Arc::new(HighFaithfulnessJudge) as Arc<dyn LlmClient>,
        0.7,
        0.7,
        Duration::from_secs(60),
        10,
    );

    let processed = worker.process_batch().await.expect("should process batch");

    assert_eq!(processed, 1);
    assert_eq!(outbox.done_count().await, 0);
    assert_eq!(outbox.failed_count().await, 1);
}
