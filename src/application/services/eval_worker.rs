// @AI-BYPASS-LENGTH
use std::sync::Arc;
use std::time::Duration;

use tracing::Instrument;

use crate::application::ports::{
    EvalEventRepository, EvalOutboxRepository, EvalResultRepository, LlmClient,
};
use crate::application::services::eval_metrics;
use crate::domain::{EvalOperationType, EvalOutboxEntry, EvalResult};

pub struct EvalWorker {
    outbox_repository: Arc<dyn EvalOutboxRepository>,
    event_repository: Arc<dyn EvalEventRepository>,
    result_repository: Arc<dyn EvalResultRepository>,
    judge: Arc<dyn LlmClient>,
    faithfulness_threshold: f32,
    poll_interval: Duration,
    batch_size: usize,
}

impl EvalWorker {
    pub fn new(
        outbox_repository: Arc<dyn EvalOutboxRepository>,
        event_repository: Arc<dyn EvalEventRepository>,
        result_repository: Arc<dyn EvalResultRepository>,
        judge: Arc<dyn LlmClient>,
        faithfulness_threshold: f32,
        poll_interval: Duration,
        batch_size: usize,
    ) -> Self {
        Self {
            outbox_repository,
            event_repository,
            result_repository,
            judge,
            faithfulness_threshold,
            poll_interval,
            batch_size,
        }
    }

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

    async fn receive_batch(&self) -> Result<Vec<EvalOutboxEntry>, EvalWorkerError> {
        self.outbox_repository
            .claim_pending(self.batch_size)
            .await
            .map_err(|e| EvalWorkerError::Outbox(e.to_string()))
    }

    async fn process_entry(&self, entry: EvalOutboxEntry) {
        let span = tracing::info_span!(
            "eval_process",
            eval_event_id = %entry.eval_event_id,
            outbox_id = %entry.id,
            correlation_id = tracing::field::Empty,
        );

        match self.score_and_persist(&entry).instrument(span).await {
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

        if let Some(cid) = &event.correlation_id {
            tracing::Span::current().record("correlation_id", cid.as_str());
        }

        match event.operation_type {
            EvalOperationType::Query => self.score_rag_query(entry, &event).await,
            EvalOperationType::AgenticRun => self.score_agentic_run(entry, &event).await,
            EvalOperationType::IngestionPdf | EvalOperationType::IngestionMp4 => {
                self.score_ingestion(entry, &event).await
            }
        }
    }

    async fn score_rag_query(
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

        let answer_relevancy = eval_metrics::compute_answer_relevancy(
            self.judge.as_ref(),
            &event.question,
            &event.generated_answer,
        )
        .await
        .map_err(|e| EvalWorkerError::Judge(e.to_string()))?;

        let context_precision = eval_metrics::compute_context_precision(
            self.judge.as_ref(),
            &event.question,
            &event.retrieved_sources,
        )
        .await
        .map_err(|e| EvalWorkerError::Judge(e.to_string()))?;

        let result = EvalResult::new(
            entry.eval_event_id,
            faithfulness,
            Some(answer_relevancy),
            Some(context_precision),
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
            faithfulness,
            answer_relevancy,
            context_precision,
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

    async fn score_agentic_run(
        &self,
        entry: &EvalOutboxEntry,
        event: &crate::domain::EvalEvent,
    ) -> Result<(), EvalWorkerError> {
        let used_tool_calls = event
            .agentic_trace
            .as_ref()
            .map(|t| !t.tool_calls.is_empty())
            .unwrap_or(false);

        let (faithfulness, context_precision) = match &event.agentic_trace {
            Some(trace) if !trace.tool_calls.is_empty() => {
                let f = eval_metrics::compute_agentic_faithfulness(
                    self.judge.as_ref(),
                    &event.generated_answer,
                    &trace.tool_calls,
                )
                .await
                .map_err(|e| EvalWorkerError::Judge(e.to_string()))?;

                let cp = eval_metrics::compute_context_precision(
                    self.judge.as_ref(),
                    &event.question,
                    &event.retrieved_sources,
                )
                .await
                .map_err(|e| EvalWorkerError::Judge(e.to_string()))?;

                (f, Some(cp))
            }
            _ => {
                // Zero-tool-call path (empty trace OR no trace at all): grade faithfulness
                // against flat context; context_precision is None — no retrieval happened.
                tracing::debug!(
                    eval_event_id = %entry.eval_event_id,
                    has_trace = event.agentic_trace.is_some(),
                    "score_agentic_run: no tool calls — flat context faithfulness, context_precision skipped"
                );
                let context = event.context_text();
                let f = eval_metrics::compute_faithfulness(
                    self.judge.as_ref(),
                    &event.generated_answer,
                    &context,
                )
                .await
                .map_err(|e| EvalWorkerError::Judge(e.to_string()))?;

                (f, None)
            }
        };

        let answer_relevancy = eval_metrics::compute_answer_relevancy(
            self.judge.as_ref(),
            &event.question,
            &event.generated_answer,
        )
        .await
        .map_err(|e| EvalWorkerError::Judge(e.to_string()))?;

        let result = EvalResult::new(
            entry.eval_event_id,
            faithfulness,
            Some(answer_relevancy),
            context_precision,
            None,
            None,
            self.faithfulness_threshold,
        );

        self.result_repository
            .save(&result)
            .await
            .map_err(|e| EvalWorkerError::ResultRepository(e.to_string()))?;

        let reflection_score = event
            .agentic_trace
            .as_ref()
            .and_then(|t| t.reflection_score);

        tracing::info!(
            eval_event_id = %entry.eval_event_id,
            operation_type = event.operation_type.as_str(),
            faithfulness,
            answer_relevancy,
            context_precision = ?context_precision,
            below_threshold = result.below_threshold,
            reflection_score = ?reflection_score,
            used_tool_calls,
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
            chunk_count,
            non_empty = chunk_count > 0,
            "eval.result"
        );

        self.outbox_repository
            .mark_done(entry.id)
            .await
            .map_err(|e| EvalWorkerError::Outbox(e.to_string()))?;

        Ok(())
    }

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
