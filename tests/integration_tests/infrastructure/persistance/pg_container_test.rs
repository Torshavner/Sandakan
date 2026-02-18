use crate::helpers::TestPostgres;

#[tokio::test]
async fn given_test_suite_starting_up_when_initializing_postgres_container_then_instance_is_available_and_migrations_run()
 {
    let test_pg = TestPostgres::new().await;

    let result = sqlx::query!("SELECT COUNT(*) as count FROM jobs")
        .fetch_one(&test_pg.pool)
        .await
        .expect("Failed to query jobs table");

    assert_eq!(result.count.unwrap_or(0), 0, "Jobs table should be empty");
}
