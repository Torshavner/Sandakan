use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::EvalEventId;

/// Status lifecycle for outbox entries: Pending → Processing → Done | Failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvalOutboxStatus {
    Pending,
    Processing,
    Done,
    Failed,
}

impl EvalOutboxStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

impl std::fmt::Display for EvalOutboxStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for EvalOutboxStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "done" => Ok(Self::Done),
            "failed" => Ok(Self::Failed),
            other => Err(format!("unknown eval outbox status: {other}")),
        }
    }
}

/// A durable outbox entry linking to an EvalEvent. The background EvalWorker
/// claims pending entries, runs LLM-as-judge scoring, then marks them done/failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalOutboxEntry {
    pub id: Uuid,
    pub eval_event_id: EvalEventId,
    pub status: EvalOutboxStatus,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl EvalOutboxEntry {
    pub fn new(eval_event_id: EvalEventId) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            eval_event_id,
            status: EvalOutboxStatus::Pending,
            error: None,
            created_at: now,
            updated_at: now,
        }
    }
}
