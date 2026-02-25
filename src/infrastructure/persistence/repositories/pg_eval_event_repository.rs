use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::application::ports::{EvalEventError, EvalEventRepository};
use crate::domain::{EvalEvent, EvalEventId, EvalOperationType, EvalSource};

pub struct PgEvalEventRepository {
    pool: PgPool,
}

impl PgEvalEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

struct EvalEventRow {
    id: Uuid,
    timestamp: DateTime<Utc>,
    question: String,
    generated_answer: String,
    retrieved_sources: serde_json::Value,
    model_config: String,
    operation_type: String,
    correlation_id: Option<String>,
}

fn parse_operation_type(s: &str) -> EvalOperationType {
    match s {
        "agentic_run" => EvalOperationType::AgenticRun,
        "ingestion_pdf" => EvalOperationType::IngestionPdf,
        "ingestion_mp4" => EvalOperationType::IngestionMp4,
        // default covers "query" and any legacy unknown values
        _ => EvalOperationType::Query,
    }
}

fn row_to_event(r: EvalEventRow) -> Result<EvalEvent, EvalEventError> {
    let sources: Vec<EvalSource> = serde_json::from_value(r.retrieved_sources)
        .map_err(|e| EvalEventError::Serialization(e.to_string()))?;
    Ok(EvalEvent {
        id: EvalEventId::from_uuid(r.id),
        timestamp: r.timestamp,
        question: r.question,
        generated_answer: r.generated_answer,
        retrieved_sources: sources,
        model_config: r.model_config,
        operation_type: parse_operation_type(&r.operation_type),
        correlation_id: r.correlation_id,
    })
}

#[async_trait]
impl EvalEventRepository for PgEvalEventRepository {
    #[instrument(skip(self, event), fields(event_id = %event.id))]
    async fn record(&self, event: &EvalEvent) -> Result<(), EvalEventError> {
        let id = event.id.as_uuid();
        let sources = serde_json::to_value(&event.retrieved_sources)
            .map_err(|e| EvalEventError::Serialization(e.to_string()))?;

        sqlx::query!(
            r#"
            INSERT INTO eval_events (id, timestamp, question, generated_answer, retrieved_sources, model_config, operation_type, correlation_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO NOTHING
            "#,
            id,
            event.timestamp,
            event.question,
            event.generated_answer,
            sources,
            event.model_config,
            event.operation_type.as_str(),
            event.correlation_id.as_deref()
        )
        .execute(&self.pool)
        .await
        .map_err(|e: sqlx::Error| EvalEventError::Serialization(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self), fields(event_id = %id))]
    async fn get(&self, id: EvalEventId) -> Result<Option<EvalEvent>, EvalEventError> {
        let uuid = id.as_uuid();
        let row = sqlx::query_as!(
            EvalEventRow,
            r#"
            SELECT id, timestamp, question, generated_answer, retrieved_sources, model_config, operation_type, correlation_id
            FROM eval_events
            WHERE id = $1
            "#,
            uuid
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| EvalEventError::Serialization(e.to_string()))?;

        row.map(row_to_event).transpose()
    }

    #[instrument(skip(self), fields(limit = ?limit))]
    async fn list(&self, limit: Option<usize>) -> Result<Vec<EvalEvent>, EvalEventError> {
        let rows = sqlx::query_as!(
            EvalEventRow,
            r#"
            SELECT id, timestamp, question, generated_answer, retrieved_sources, model_config, operation_type, correlation_id
            FROM eval_events
            ORDER BY timestamp DESC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EvalEventError::Serialization(e.to_string()))?;

        let iter: Box<dyn Iterator<Item = EvalEventRow>> = match limit {
            Some(n) => Box::new(rows.into_iter().take(n)),
            None => Box::new(rows.into_iter()),
        };

        iter.map(row_to_event).collect()
    }

    #[instrument(skip(self), fields(n = %n))]
    async fn sample(&self, n: usize) -> Result<Vec<EvalEvent>, EvalEventError> {
        let rows = sqlx::query_as!(
            EvalEventRow,
            r#"
            SELECT id, timestamp, question, generated_answer, retrieved_sources, model_config, operation_type, correlation_id
            FROM eval_events
            ORDER BY RANDOM()
            LIMIT $1
            "#,
            n as i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EvalEventError::Serialization(e.to_string()))?;

        rows.into_iter().map(row_to_event).collect()
    }
}
