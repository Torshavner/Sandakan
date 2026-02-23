use std::collections::HashSet;

use crate::application::ports::{Embedder, EmbedderError, LlmClient, LlmClientError};
use crate::domain::EvalSource;

/// Context recall: fraction of expected source pages present in retrieved sources.
/// Returns 1.0 when `expected_pages` is `None` or empty (no penalty without ground truth).
pub fn compute_context_recall(
    expected_pages: Option<&[u32]>,
    retrieved_sources: &[EvalSource],
) -> f32 {
    let Some(pages) = expected_pages else {
        return 1.0;
    };
    if pages.is_empty() {
        return 1.0;
    }
    let retrieved: HashSet<u32> = retrieved_sources.iter().filter_map(|s| s.page).collect();
    let found = pages.iter().filter(|p| retrieved.contains(p)).count();
    found as f32 / pages.len() as f32
}

/// Answer correctness: cosine similarity between embeddings of generated and expected answers.
pub async fn compute_correctness(
    embedder: &dyn Embedder,
    generated_answer: &str,
    expected_answer: &str,
) -> Result<f32, EmbedderError> {
    let embeddings = embedder
        .embed_batch(&[generated_answer, expected_answer])
        .await?;
    if embeddings.len() < 2 {
        return Err(EmbedderError::InvalidResponse(
            "expected 2 embeddings from embed_batch".to_string(),
        ));
    }
    // Index 0 = generated, index 1 = expected (same order as input)
    Ok(embeddings[0].cosine_similarity(&embeddings[1]))
}

/// Answer faithfulness via LLM-as-judge.
/// The retrieved context is passed as `context`; the judge prompt goes as `prompt`.
/// Returns a score in `0.0..=1.0` or an error if the model response is unparseable.
pub async fn compute_faithfulness(
    judge: &dyn LlmClient,
    generated_answer: &str,
    context: &str,
) -> Result<f32, LlmClientError> {
    let prompt = build_faithfulness_prompt(generated_answer);
    let raw = judge.complete(&prompt, context).await?;
    parse_faithfulness_score(&raw).ok_or_else(|| {
        LlmClientError::InvalidResponse(format!(
            "judge returned non-numeric faithfulness score: '{}'",
            raw.trim()
        ))
    })
}

fn build_faithfulness_prompt(generated_answer: &str) -> String {
    format!(
        "You are a strict factual grounding evaluator.\n\
         Your task: Determine whether the ANSWER below is fully supported by the CONTEXT \
         provided. Do not use any external knowledge.\n\n\
         ANSWER:\n{answer}\n\n\
         Instructions:\n\
         - Score 1.0 if every factual claim in the ANSWER can be directly traced to the CONTEXT.\n\
         - Score 0.0 if the ANSWER contains any claim not present in the CONTEXT.\n\
         - Use intermediate values (e.g. 0.5) for partial grounding.\n\
         - Reply with ONLY a single decimal number between 0.0 and 1.0. \
           No explanation. No other text.\n\n\
         Score:",
        answer = generated_answer
    )
}

/// Parses the first line of `raw` as an f32 in `[0.0, 1.0]`.
/// Returns `None` if the value cannot be parsed or is out of range.
pub(crate) fn parse_faithfulness_score(raw: &str) -> Option<f32> {
    raw.trim()
        .lines()
        .next()?
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|s| (0.0f32..=1.0f32).contains(s))
}
