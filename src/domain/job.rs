use super::{DocumentId, JobId, JobStatus};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Job {
    pub id: JobId,
    pub document_id: Option<DocumentId>,
    pub status: JobStatus,
    pub job_type: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Job {
    pub fn new(document_id: Option<DocumentId>, job_type: String) -> Self {
        let now = Utc::now();
        Self {
            id: JobId::new(),
            document_id,
            status: JobStatus::Queued,
            job_type,
            error_message: None,
            created_at: now,
            updated_at: now,
        }
    }
}
