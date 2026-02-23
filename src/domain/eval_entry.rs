use serde::{Deserialize, Serialize};

/// Ground-truth record for offline evaluation, loaded from a JSONL file.
/// Used by the `evaluate` CLI binary and `EvalMetrics` optional scoring functions
/// (`compute_context_recall`, `compute_correctness`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalEntry {
    pub question: String,
    pub expected_answer: String,
    #[serde(default)]
    pub expected_source_pages: Option<Vec<u32>>,
}
