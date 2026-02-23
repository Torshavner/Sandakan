use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::application::ports::{EvalOutboxError, EvalOutboxRepository};
use crate::domain::{EvalEventId, EvalOutboxEntry, EvalOutboxStatus};

pub struct PgEvalOutboxRepository {
    pool: PgPool,
}

impl PgEvalOutboxRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

struct OutboxRow {
    id: Uuid,
    eval_event_id: Uuid,
    status: String,
    error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn row_to_entry(r: OutboxRow) -> Result<EvalOutboxEntry, EvalOutboxError> {
    let status: EvalOutboxStatus = r
        .status
        .parse()
        .map_err(|e: String| EvalOutboxError::Serialization(e))?;
    Ok(EvalOutboxEntry {
        id: r.id,
        eval_event_id: EvalEventId::from_uuid(r.eval_event_id),
        status,
        error: r.error,
        created_at: r.created_at,
        updated_at: r.updated_at,
    })
}

#[async_trait]
impl EvalOutboxRepository for PgEvalOutboxRepository {
    #[instrument(skip(self), fields(eval_event_id = %eval_event_id))]
    async fn enqueue(&self, eval_event_id: EvalEventId) -> Result<(), EvalOutboxError> {
        let event_id = eval_event_id.as_uuid();
        sqlx::query!(
            r#"
            INSERT INTO eval_outbox (eval_event_id)
            VALUES ($1)
            "#,
            event_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| EvalOutboxError::Database(e.to_string()))?;
        Ok(())
    }

    #[instrument(skip(self), fields(batch_size = %batch_size))]
    async fn claim_pending(
        &self,
        batch_size: usize,
    ) -> Result<Vec<EvalOutboxEntry>, EvalOutboxError> {
        let rows = sqlx::query_as!(
            OutboxRow,
            r#"
            UPDATE eval_outbox
            SET status = 'processing', updated_at = NOW()
            WHERE id IN (
                SELECT id FROM eval_outbox
                WHERE status = 'pending'
                ORDER BY created_at
                LIMIT $1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING id, eval_event_id, status, error, created_at, updated_at
            "#,
            batch_size as i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| EvalOutboxError::Database(e.to_string()))?;

        rows.into_iter().map(row_to_entry).collect()
    }

    #[instrument(skip(self), fields(outbox_id = %id))]
    async fn mark_done(&self, id: Uuid) -> Result<(), EvalOutboxError> {
        sqlx::query!(
            r#"
            UPDATE eval_outbox
            SET status = 'done', updated_at = NOW()
            WHERE id = $1
            "#,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| EvalOutboxError::Database(e.to_string()))?;
        Ok(())
    }

    #[instrument(skip(self), fields(outbox_id = %id))]
    async fn mark_failed(&self, id: Uuid, error: &str) -> Result<(), EvalOutboxError> {
        sqlx::query!(
            r#"
            UPDATE eval_outbox
            SET status = 'failed', error = $2, updated_at = NOW()
            WHERE id = $1
            "#,
            id,
            error
        )
        .execute(&self.pool)
        .await
        .map_err(|e| EvalOutboxError::Database(e.to_string()))?;
        Ok(())
    }
}
