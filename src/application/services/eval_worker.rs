use std::sync::Arc;
use std::time::Duration;

use crate::application::ports::{
    Embedder, EvalEventRepository, EvalOutboxRepository, EvalResultRepository, LlmClient,
};
use crate::application::services::eval_metrics;
use crate::domain::{EvalOperationType, EvalOutboxEntry, EvalResult};

pub struct EvalWorker {
    outbox_repository: Arc<dyn EvalOutboxRepository>,
    event_repository: Arc<dyn EvalEventRepository>,
    result_repository: Arc<dyn EvalResultRepository>,
    _embedder: Arc<dyn Embedder>,
    judge: Arc<dyn LlmClient>,
    faithfulness_threshold: f32,
    _correctness_threshold: f32,
    poll_interval: Duration,
    batch_size: usize,
}

impl EvalWorker {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        outbox_repository: Arc<dyn EvalOutboxRepository>,
        event_repository: Arc<dyn EvalEventRepository>,
        result_repository: Arc<dyn EvalResultRepository>,
        embedder: Arc<dyn Embedder>,
        judge: Arc<dyn LlmClient>,
        faithfulness_threshold: f32,
        correctness_threshold: f32,
        poll_interval: Duration,
        batch_size: usize,
    ) -> Self {
        Self {
            outbox_repository,
            event_repository,
            result_repository,
            _embedder: embedder,
            judge,
            faithfulness_threshold,
            _correctness_threshold: correctness_threshold,
            poll_interval,
            batch_size,
        }
    }

    /// Actor loop — transport concern (interval + claim_pending).
    /// Post-US-017: becomes `while let Some(entry) = subscriber.receive().await`
    pub async fn run(self) {
        tracing::info!("EvalWorker started");
        let mut interval = tokio::time::interval(self.poll_interval);
        loop {
            interval.tick().await;
            match self.receive_batch().await {
                Ok(entries) => {
                    for entry in entries {
                        self.process_entry(entry).await;
                    }
                }
                Err(e) => tracing::error!(error = %e, "EvalWorker receive failed"),
            }
        }
    }

    /// Transport concern — wraps outbox polling.
    /// Post-US-017: extracted into OutboxSubscriber<EvalOutboxEntry>::receive()
    async fn receive_batch(&self) -> Result<Vec<EvalOutboxEntry>, EvalWorkerError> {
        self.outbox_repository
            .claim_pending(self.batch_size)
            .await
            .map_err(|e| EvalWorkerError::Outbox(e.to_string()))
    }

    /// Pure business logic — evaluates one entry, persists result, emits metrics, marks done/failed.
    /// This method is stable across the US-017 migration.
    async fn process_entry(&self, entry: EvalOutboxEntry) {
        let span = tracing::info_span!(
            "eval_process",
            eval_event_id = %entry.eval_event_id,
            outbox_id = %entry.id,
        );
        let _guard = span.enter();

        match self.score_and_persist(&entry).await {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!(error = %e, "Eval scoring failed");
                if let Err(mark_err) = self
                    .outbox_repository
                    .mark_failed(entry.id, &e.to_string())
                    .await
                {
                    tracing::error!(error = %mark_err, "Failed to mark outbox entry as failed");
                }
            }
        }
    }

    async fn score_and_persist(&self, entry: &EvalOutboxEntry) -> Result<(), EvalWorkerError> {
        let event = self
            .event_repository
            .get(entry.eval_event_id)
            .await
            .map_err(|e| EvalWorkerError::EventRepository(e.to_string()))?
            .ok_or_else(|| {
                EvalWorkerError::EventRepository(format!(
                    "eval event {} not found",
                    entry.eval_event_id
                ))
            })?;

        match event.operation_type {
            EvalOperationType::Query | EvalOperationType::AgenticRun => {
                self.score_llm_based(entry, &event).await
            }
            EvalOperationType::IngestionPdf | EvalOperationType::IngestionMp4 => {
                self.score_ingestion(entry, &event).await
            }
        }
    }

    async fn score_llm_based(
        &self,
        entry: &EvalOutboxEntry,
        event: &crate::domain::EvalEvent,
    ) -> Result<(), EvalWorkerError> {
        let context = event.context_text();

        let faithfulness = eval_metrics::compute_faithfulness(
            self.judge.as_ref(),
            &event.generated_answer,
            &context,
        )
        .await
        .map_err(|e| EvalWorkerError::Judge(e.to_string()))?;

        // context_recall and correctness require ground-truth; not available in the online worker path.
        let result = EvalResult::new(
            entry.eval_event_id,
            faithfulness,
            None,
            None,
            self.faithfulness_threshold,
        );

        self.result_repository
            .save(&result)
            .await
            .map_err(|e| EvalWorkerError::ResultRepository(e.to_string()))?;

        tracing::info!(
            eval_event_id = %entry.eval_event_id,
            operation_type = event.operation_type.as_str(),
            faithfulness = faithfulness,
            below_threshold = result.below_threshold,
            model_config = %event.model_config,
            "eval.result"
        );

        self.outbox_repository
            .mark_done(entry.id)
            .await
            .map_err(|e| EvalWorkerError::Outbox(e.to_string()))?;

        Ok(())
    }

    async fn score_ingestion(
        &self,
        entry: &EvalOutboxEntry,
        event: &crate::domain::EvalEvent,
    ) -> Result<(), EvalWorkerError> {
        let chunk_count: usize = event.generated_answer.parse().unwrap_or(0);
        let faithfulness = if chunk_count > 0 { 1.0_f32 } else { 0.0_f32 };

        let result = EvalResult::new(
            entry.eval_event_id,
            faithfulness,
            None,
            None,
            self.faithfulness_threshold,
        );

        self.result_repository
            .save(&result)
            .await
            .map_err(|e| EvalWorkerError::ResultRepository(e.to_string()))?;

        tracing::info!(
            eval_event_id = %entry.eval_event_id,
            operation_type = event.operation_type.as_str(),
            chunk_count = chunk_count,
            non_empty = chunk_count > 0,
            "eval.result"
        );

        self.outbox_repository
            .mark_done(entry.id)
            .await
            .map_err(|e| EvalWorkerError::Outbox(e.to_string()))?;

        Ok(())
    }

    /// Process a single batch — useful for testing without the interval loop.
    pub async fn process_batch(&self) -> Result<usize, EvalWorkerError> {
        let entries = self.receive_batch().await?;
        let count = entries.len();
        if count > 0 {
            tracing::debug!(batch_size = count, "Processing eval outbox batch");
        }
        for entry in entries {
            self.process_entry(entry).await;
        }
        Ok(count)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EvalWorkerError {
    #[error("outbox: {0}")]
    Outbox(String),
    #[error("event repository: {0}")]
    EventRepository(String),
    #[error("result repository: {0}")]
    ResultRepository(String),
    #[error("judge: {0}")]
    Judge(String),
}
