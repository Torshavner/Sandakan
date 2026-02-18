use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use tracing::instrument;

use crate::application::ports::{JobRepository, RepositoryError};
use crate::domain::{DocumentId, Job, JobId, JobStatus};

pub struct PgJobRepository {
    pool: PgPool,
}

impl PgJobRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl JobRepository for PgJobRepository {
    #[instrument(skip(self, job), fields(job_id = %job.id.as_uuid()))]
    async fn create(&self, job: &Job) -> Result<(), RepositoryError> {
        let job_id = job.id.as_uuid();
        let document_id = job.document_id.map(|id| id.as_uuid());
        let status = job.status.as_str();

        sqlx::query!(
            r#"
            INSERT INTO jobs (id, document_id, status, job_type, error_message, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            job_id,
            document_id,
            status,
            job.job_type,
            job.error_message,
            job.created_at,
            job.updated_at
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::QueryFailed(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self), fields(job_id = %id.as_uuid()))]
    async fn get_by_id(&self, id: JobId) -> Result<Option<Job>, RepositoryError> {
        let job_id = id.as_uuid();

        let row = sqlx::query!(
            r#"
            SELECT id, document_id, status, job_type, error_message, created_at, updated_at
            FROM jobs
            WHERE id = $1
            "#,
            job_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::QueryFailed(e.to_string()))?;

        match row {
            Some(r) => {
                let status = r
                    .status
                    .parse::<JobStatus>()
                    .map_err(RepositoryError::QueryFailed)?;

                Ok(Some(Job {
                    id: JobId::from_uuid(r.id),
                    document_id: r.document_id.map(DocumentId::from_uuid),
                    status,
                    job_type: r.job_type,
                    error_message: r.error_message,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    #[instrument(skip(self, error_message), fields(job_id = %id.as_uuid(), status = %status))]
    async fn update_status(
        &self,
        id: JobId,
        status: JobStatus,
        error_message: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let job_id = id.as_uuid();
        let status_str = status.as_str();
        let now = Utc::now();

        sqlx::query!(
            r#"
            UPDATE jobs
            SET status = $1, error_message = $2, updated_at = $3
            WHERE id = $4
            "#,
            status_str,
            error_message,
            now,
            job_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::QueryFailed(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self), fields(status = %status))]
    async fn list_by_status(&self, status: JobStatus) -> Result<Vec<Job>, RepositoryError> {
        let status_str = status.as_str();

        let rows = sqlx::query!(
            r#"
            SELECT id, document_id, status, job_type, error_message, created_at, updated_at
            FROM jobs
            WHERE status = $1
            ORDER BY created_at DESC
            "#,
            status_str
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::QueryFailed(e.to_string()))?;

        let jobs: Result<Vec<Job>, RepositoryError> = rows
            .into_iter()
            .map(|r| {
                let status = r
                    .status
                    .parse::<JobStatus>()
                    .map_err(RepositoryError::QueryFailed)?;

                Ok(Job {
                    id: JobId::from_uuid(r.id),
                    document_id: r.document_id.map(DocumentId::from_uuid),
                    status,
                    job_type: r.job_type,
                    error_message: r.error_message,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                })
            })
            .collect();

        jobs
    }
}
