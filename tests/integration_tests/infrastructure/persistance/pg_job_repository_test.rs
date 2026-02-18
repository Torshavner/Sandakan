use sandakan::application::ports::JobRepository;
use sandakan::domain::{DocumentId, Job, JobStatus};

use crate::helpers::TestPostgres;

#[tokio::test]
async fn given_new_job_when_creating_and_retrieving_then_job_is_persisted() {
    let test_pg = TestPostgres::new().await;

    let document_id = DocumentId::new();
    let job = Job::new(Some(document_id), "test_ingestion".to_string());
    let job_id = job.id;

    test_pg
        .job_repository
        .create(&job)
        .await
        .expect("Failed to create job");

    let retrieved = test_pg
        .job_repository
        .get_by_id(job_id)
        .await
        .expect("Failed to retrieve job")
        .expect("Job not found");

    assert_eq!(retrieved.id, job.id);
    assert_eq!(retrieved.document_id, job.document_id);
    assert_eq!(retrieved.status, JobStatus::Queued);
    assert_eq!(retrieved.job_type, job.job_type);
}

#[tokio::test]
async fn given_existing_job_when_updating_status_then_status_is_changed() {
    let test_pg = TestPostgres::new().await;

    let job = Job::new(None, "test_job".to_string());
    let job_id = job.id;

    test_pg
        .job_repository
        .create(&job)
        .await
        .expect("Failed to create job");

    test_pg
        .job_repository
        .update_status(job_id, JobStatus::Processing, None)
        .await
        .expect("Failed to update status");

    let retrieved = test_pg
        .job_repository
        .get_by_id(job_id)
        .await
        .expect("Failed to retrieve job")
        .expect("Job not found");

    assert_eq!(retrieved.status, JobStatus::Processing);
}

#[tokio::test]
async fn given_jobs_with_different_statuses_when_listing_by_status_then_only_matching_jobs_returned()
 {
    let test_pg = TestPostgres::new().await;

    let job1 = Job::new(None, "job1".to_string());
    let job2 = Job::new(None, "job2".to_string());
    let job3 = Job::new(None, "job3".to_string());

    test_pg.job_repository.create(&job1).await.unwrap();
    test_pg.job_repository.create(&job2).await.unwrap();
    test_pg.job_repository.create(&job3).await.unwrap();

    test_pg
        .job_repository
        .update_status(job2.id, JobStatus::Completed, None)
        .await
        .unwrap();
    test_pg
        .job_repository
        .update_status(job3.id, JobStatus::Completed, None)
        .await
        .unwrap();

    let completed_jobs = test_pg
        .job_repository
        .list_by_status(JobStatus::Completed)
        .await
        .expect("Failed to list jobs");

    assert_eq!(completed_jobs.len(), 2);
    assert!(
        completed_jobs
            .iter()
            .all(|j| j.status == JobStatus::Completed)
    );
}

#[tokio::test]
async fn given_nonexistent_job_id_when_retrieving_then_returns_none() {
    let test_pg = TestPostgres::new().await;

    let nonexistent_id = sandakan::domain::JobId::new();
    let result = test_pg
        .job_repository
        .get_by_id(nonexistent_id)
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}
