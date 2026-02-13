use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use tracing::{info, instrument, warn};

use crate::application::ports::RepositoryError;

#[instrument(skip(url))]
pub async fn create_pool(url: &str, max_connections: u32) -> Result<PgPool, RepositoryError> {
    let mut retries = 5;
    let mut delay = Duration::from_millis(500);

    loop {
        match PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(url)
            .await
        {
            Ok(pool) => {
                info!("PostgreSQL connection pool established");
                return Ok(pool);
            }
            Err(e) if retries > 0 => {
                retries -= 1;
                warn!(
                    error = %e,
                    retries_left = retries,
                    delay_ms = delay.as_millis(),
                    "PostgreSQL connection failed, retrying"
                );
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
            Err(e) => {
                return Err(RepositoryError::ConnectionFailed(e.to_string()));
            }
        }
    }
}
