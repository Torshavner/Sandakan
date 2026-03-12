use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::EvalEventId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvalResultId(Uuid);

impl EvalResultId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for EvalResultId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EvalResultId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Persisted outcome of one LLM-as-judge evaluation run for a single `EvalEvent`.
///
/// `context_recall` and `correctness` are `None` when no ground-truth is available.
/// `answer_relevancy` and `context_precision` are populated on the online path for
/// Query and AgenticRun events; `None` for ingestion events.
///
/// `below_threshold` is pre-computed at save time so dashboards can filter cheaply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub id: EvalResultId,
    pub eval_event_id: EvalEventId,
    pub faithfulness: f32,
    pub answer_relevancy: Option<f32>,
    pub context_precision: Option<f32>,
    pub context_recall: Option<f32>,
    pub correctness: Option<f32>,
    pub below_threshold: bool,
    pub computed_at: DateTime<Utc>,
}

impl EvalResult {
    pub fn new(
        eval_event_id: EvalEventId,
        faithfulness: f32,
        answer_relevancy: Option<f32>,
        context_precision: Option<f32>,
        context_recall: Option<f32>,
        correctness: Option<f32>,
        faithfulness_threshold: f32,
    ) -> Self {
        Self {
            id: EvalResultId::new(),
            eval_event_id,
            faithfulness,
            answer_relevancy,
            context_precision,
            context_recall,
            correctness,
            below_threshold: faithfulness < faithfulness_threshold,
            computed_at: Utc::now(),
        }
    }
}
