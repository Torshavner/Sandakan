use sandakan::infrastructure::persistence::{PgConversationRepository, PgJobRepository};
use sqlx::PgPool;
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt, core::ContainerPort, runners::AsyncRunner,
};

pub struct TestPostgres {
    pub pool: PgPool,
    pub job_repository: PgJobRepository,
    pub conversation_repository: PgConversationRepository,
    _container: ContainerAsync<GenericImage>,
}

impl TestPostgres {
    pub async fn new() -> Self {
        let postgres_image = GenericImage::new("postgres", "16")
            .with_exposed_port(ContainerPort::Tcp(5432))
            .with_env_var("POSTGRES_USER", "test")
            .with_env_var("POSTGRES_PASSWORD", "test")
            .with_env_var("POSTGRES_DB", "testdb");

        let container = postgres_image
            .start()
            .await
            .expect("Failed to start PostgreSQL container");

        let host_port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get PostgreSQL port");

        let database_url = format!("postgres://test:test@localhost:{}/testdb", host_port);

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let pool = sqlx::PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to PostgreSQL");

        sqlx::migrate!()
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        let job_repository = PgJobRepository::new(pool.clone());
        let conversation_repository = PgConversationRepository::new(pool.clone());

        Self {
            pool: pool.clone(),
            job_repository,
            conversation_repository,
            _container: container,
        }
    }
}

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
