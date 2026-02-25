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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalOperationType {
    #[default]
    Query,
    AgenticRun,
    IngestionPdf,
    IngestionMp4,
}

impl EvalOperationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::AgenticRun => "agentic_run",
            Self::IngestionPdf => "ingestion_pdf",
            Self::IngestionMp4 => "ingestion_mp4",
        }
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
    pub operation_type: EvalOperationType,
    /// Propagated from the originating HTTP request so the EvalWorker can link
    /// its scoring span back to the request trace in Tempo/Grafana.
    #[serde(default)]
    pub correlation_id: Option<String>,
}

impl EvalEvent {
    /// RAG query event — backward-compatible default.
    pub fn new(
        question: &str,
        generated_answer: &str,
        retrieved_sources: Vec<EvalSource>,
        model_config: &str,
        correlation_id: Option<String>,
    ) -> Self {
        Self {
            id: EvalEventId::new(),
            timestamp: Utc::now(),
            question: question.to_string(),
            generated_answer: generated_answer.to_string(),
            retrieved_sources,
            model_config: model_config.to_string(),
            operation_type: EvalOperationType::Query,
            correlation_id,
        }
    }

    /// Agentic turn event — used by `AgentService`.
    pub fn new_agentic(
        question: &str,
        generated_answer: &str,
        retrieved_sources: Vec<EvalSource>,
        model_config: &str,
        correlation_id: Option<String>,
    ) -> Self {
        Self {
            operation_type: EvalOperationType::AgenticRun,
            ..Self::new(
                question,
                generated_answer,
                retrieved_sources,
                model_config,
                correlation_id,
            )
        }
    }

    /// Ingestion pipeline event — used by `IngestionWorker`.
    ///
    /// `description` is the file name; `generated_answer` encodes `chunk_count`
    /// so the worker can score structural validity without an LLM call.
    pub fn new_ingestion(
        operation_type: EvalOperationType,
        description: &str,
        chunk_count: usize,
        model_config: &str,
        correlation_id: Option<String>,
    ) -> Self {
        Self {
            id: EvalEventId::new(),
            timestamp: Utc::now(),
            question: description.to_string(),
            generated_answer: chunk_count.to_string(),
            retrieved_sources: vec![],
            model_config: model_config.to_string(),
            operation_type,
            correlation_id,
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
