use std::path::PathBuf;

use async_trait::async_trait;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

use crate::application::ports::{EvalEventError, EvalEventRepository};
use crate::domain::{EvalEvent, EvalEventId};

pub struct JsonlEvalEventRepository {
    path: PathBuf,
}

impl JsonlEvalEventRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    async fn read_all(&self) -> Result<Vec<EvalEvent>, EvalEventError> {
        match tokio::fs::read_to_string(&self.path).await {
            Ok(content) => {
                let mut events = Vec::new();
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let event: EvalEvent = serde_json::from_str(trimmed)
                        .map_err(|e| EvalEventError::Serialization(e.to_string()))?;
                    events.push(event);
                }
                Ok(events)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(EvalEventError::Io(e)),
        }
    }
}

#[async_trait]
impl EvalEventRepository for JsonlEvalEventRepository {
    async fn record(&self, event: &EvalEvent) -> Result<(), EvalEventError> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut line = serde_json::to_string(event)
            .map_err(|e| EvalEventError::Serialization(e.to_string()))?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        file.write_all(line.as_bytes()).await?;
        Ok(())
    }

    async fn get(&self, id: EvalEventId) -> Result<Option<EvalEvent>, EvalEventError> {
        let events = self.read_all().await?;
        Ok(events.into_iter().find(|e| e.id == id))
    }

    async fn list(&self, limit: Option<usize>) -> Result<Vec<EvalEvent>, EvalEventError> {
        let events = self.read_all().await?;
        Ok(match limit {
            Some(n) => events.into_iter().take(n).collect(),
            None => events,
        })
    }

    async fn sample(&self, n: usize) -> Result<Vec<EvalEvent>, EvalEventError> {
        let mut events = self.read_all().await?;
        if events.len() <= n {
            return Ok(events);
        }
        // Fisher-Yates partial shuffle to pick n random elements
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::SystemTime;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(42);
        let mut rng_state = seed as u64;
        for i in 0..n {
            let mut hasher = DefaultHasher::new();
            rng_state.hash(&mut hasher);
            rng_state = hasher.finish();
            let j = i + (rng_state as usize % (events.len() - i));
            events.swap(i, j);
        }
        events.truncate(n);
        Ok(events)
    }
}
