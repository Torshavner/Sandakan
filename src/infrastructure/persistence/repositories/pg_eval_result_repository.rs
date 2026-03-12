use async_trait::async_trait;
use sqlx::PgPool;
use tracing::instrument;

use crate::application::ports::{EvalResultError, EvalResultRepository};
use crate::domain::EvalResult;

pub struct PgEvalResultRepository {
    pool: PgPool,
}

impl PgEvalResultRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EvalResultRepository for PgEvalResultRepository {
    #[instrument(skip(self, result), fields(result_id = %result.id))]
    async fn save(&self, result: &EvalResult) -> Result<(), EvalResultError> {
        let id = result.id.as_uuid();
        let eval_event_id = result.eval_event_id.as_uuid();

        sqlx::query!(
            r#"
            INSERT INTO eval_results
                (id, eval_event_id, question, generated_answer, eval_description,
                 faithfulness, answer_relevancy, context_precision,
                 context_recall, correctness, below_threshold, computed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (eval_event_id) DO NOTHING
            "#,
            id,
            eval_event_id,
            result.question,
            result.generated_answer,
            result.eval_description,
            result.faithfulness,
            result.answer_relevancy,
            result.context_precision,
            result.context_recall,
            result.correctness,
            result.below_threshold,
            result.computed_at,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| EvalResultError::Database(e.to_string()))?;

        Ok(())
    }
}
