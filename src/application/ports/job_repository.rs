use crate::domain::{Job, JobId, JobStatus};
use async_trait::async_trait;

use super::RepositoryError;

#[async_trait]
pub trait JobRepository: Send + Sync {
    async fn create(&self, job: &Job) -> Result<(), RepositoryError>;

    async fn get_by_id(&self, id: JobId) -> Result<Option<Job>, RepositoryError>;

    async fn update_status(
        &self,
        id: JobId,
        status: JobStatus,
        error_message: Option<&str>,
    ) -> Result<(), RepositoryError>;

    async fn list_by_status(&self, status: JobStatus) -> Result<Vec<Job>, RepositoryError>;
}
