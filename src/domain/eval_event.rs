use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvalEventId(Uuid);

impl EvalEventId {
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

impl Default for EvalEventId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EvalEventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSource {
    pub text: String,
    pub page: Option<u32>,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalEvent {
    pub id: EvalEventId,
    pub timestamp: DateTime<Utc>,
    pub question: String,
    pub generated_answer: String,
    pub retrieved_sources: Vec<EvalSource>,
    pub model_config: String,
}

impl EvalEvent {
    pub fn new(
        question: &str,
        generated_answer: &str,
        retrieved_sources: Vec<EvalSource>,
        model_config: &str,
    ) -> Self {
        Self {
            id: EvalEventId::new(),
            timestamp: Utc::now(),
            question: question.to_string(),
            generated_answer: generated_answer.to_string(),
            retrieved_sources,
            model_config: model_config.to_string(),
        }
    }

    pub fn context_text(&self) -> String {
        self.retrieved_sources
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}
