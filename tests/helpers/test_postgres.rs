use sandakan::infrastructure::persistence::{PgConversationRepository, PgJobRepository};
use sqlx::PgPool;
use std::time::Duration;
use testcontainers::core::ContainerPort;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};

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

        let pool = wait_for_pg_connection(&database_url).await;

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

async fn wait_for_pg_connection(url: &str) -> PgPool {
    let max_retries = 10;
    let mut delay = Duration::from_millis(500);

    for attempt in 1..=max_retries {
        match sqlx::PgPool::connect(url).await {
            Ok(pool) => {
                eprintln!("PostgreSQL ready after attempt {attempt}");
                return pool;
            }
            Err(e) if attempt < max_retries => {
                eprintln!(
                    "PostgreSQL not ready (attempt {attempt}/{max_retries}): {e}, retrying in {}ms",
                    delay.as_millis()
                );
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(5));
            }
            Err(e) => {
                panic!("Failed to connect to PostgreSQL after {max_retries} attempts: {e}");
            }
        }
    }
    unreachable!()
}
