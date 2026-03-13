use std::collections::HashSet;

use tracing::Instrument;

use crate::application::ports::{Embedder, EmbedderError, LlmClient, LlmClientError};
use crate::domain::{EvalSource, ToolCallTrace};

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
    tracing::debug!(
        answer_len = generated_answer.len(),
        context_len = context.len(),
        "judge.faithfulness calling LLM"
    );
    let raw = judge
        .complete(&prompt, context)
        .instrument(tracing::debug_span!("judge.faithfulness"))
        .await?;
    tracing::debug!(raw_response = %raw.trim(), "judge.faithfulness raw response");
    let score = parse_faithfulness_score(&raw);
    if score.is_some() && raw.trim().parse::<f32>().is_err() {
        tracing::warn!(raw_response = %raw.trim(), "judge.faithfulness: extracted score from noisy response");
    }
    score.ok_or_else(|| {
        LlmClientError::InvalidResponse(format!(
            "judge returned non-numeric faithfulness score: '{}'",
            raw.trim()
        ))
    })
}

/// Agentic faithfulness: judge whether the final answer is grounded in tool outputs.
/// Presents each tool call/result pair and asks the judge to score consistency.
pub async fn compute_agentic_faithfulness(
    judge: &dyn LlmClient,
    generated_answer: &str,
    tool_traces: &[ToolCallTrace],
) -> Result<f32, LlmClientError> {
    let prompt = build_agentic_faithfulness_prompt(generated_answer, tool_traces);
    tracing::debug!(
        answer_len = generated_answer.len(),
        tool_calls = tool_traces.len(),
        "judge.agentic_faithfulness calling LLM"
    );
    let raw = judge
        .complete(&prompt, "")
        .instrument(tracing::debug_span!("judge.agentic_faithfulness"))
        .await?;
    tracing::debug!(raw_response = %raw.trim(), "judge.agentic_faithfulness raw response");
    let score = parse_faithfulness_score(&raw);
    if score.is_some() && raw.trim().parse::<f32>().is_err() {
        tracing::warn!(raw_response = %raw.trim(), "judge.agentic_faithfulness: extracted score from noisy response");
    }
    score.ok_or_else(|| {
        LlmClientError::InvalidResponse(format!(
            "judge returned non-numeric agentic faithfulness score: '{}'",
            raw.trim()
        ))
    })
}

fn build_agentic_faithfulness_prompt(
    generated_answer: &str,
    tool_traces: &[ToolCallTrace],
) -> String {
    let tool_context = if tool_traces.is_empty() {
        "No tool calls were made.".to_string()
    } else {
        tool_traces
            .iter()
            .enumerate()
            .map(|(i, t)| {
                format!(
                    "Tool call {}:\n  Name: {}\n  Arguments: {}\n  Result: {}\n  Status: {}",
                    i + 1,
                    t.tool_name,
                    t.arguments,
                    t.result_preview,
                    if t.success {
                        "success"
                    } else {
                        "error/timeout"
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    format!(
        "You are a strict factual grounding evaluator for an AI agent.\n\
         The agent executed a series of tool calls and produced a final answer.\n\
         Your task: Determine whether the FINAL ANSWER is fully supported by the TOOL RESULTS.\n\
         Do not use any external knowledge.\n\n\
         TOOL CALLS AND RESULTS:\n{tool_context}\n\n\
         FINAL ANSWER:\n{answer}\n\n\
         Instructions:\n\
         - Score 1.0 if every factual claim in the FINAL ANSWER can be traced to the TOOL RESULTS.\n\
         - Score 0.0 if the FINAL ANSWER contains claims not present in the TOOL RESULTS (hallucination).\n\
         - Use intermediate values (e.g. 0.5) for partial grounding.\n\
         - Ignore claims that are reasonable synthesises or summaries of tool results.\n\
         - If all tool calls failed or timed out, score based on whether the answer correctly \
           acknowledges the failure.\n\
         - Reply with ONLY a single decimal number between 0.0 and 1.0. \
           No explanation. No other text.\n\n\
         Score:",
        tool_context = tool_context,
        answer = generated_answer
    )
}

/// Answer relevancy: LLM-as-judge score for how directly the answer addresses the question.
/// Returns a score in `0.0..=1.0`.
pub async fn compute_answer_relevancy(
    judge: &dyn LlmClient,
    question: &str,
    generated_answer: &str,
) -> Result<f32, LlmClientError> {
    let prompt = build_answer_relevancy_prompt(question, generated_answer);
    tracing::debug!(
        question_len = question.len(),
        answer_len = generated_answer.len(),
        "judge.answer_relevancy calling LLM"
    );
    let raw = judge
        .complete(&prompt, "")
        .instrument(tracing::debug_span!("judge.answer_relevancy"))
        .await?;
    tracing::debug!(raw_response = %raw.trim(), "judge.answer_relevancy raw response");
    let score = parse_faithfulness_score(&raw);
    if score.is_some() && raw.trim().parse::<f32>().is_err() {
        tracing::warn!(raw_response = %raw.trim(), "judge.answer_relevancy: extracted score from noisy response");
    }
    score.ok_or_else(|| {
        LlmClientError::InvalidResponse(format!(
            "judge returned non-numeric relevancy score: '{}'",
            raw.trim()
        ))
    })
}

/// Context precision: fraction of retrieved chunks judged relevant to the question by the LLM.
/// Returns a score in `0.0..=1.0`.
pub async fn compute_context_precision(
    judge: &dyn LlmClient,
    question: &str,
    sources: &[EvalSource],
) -> Result<f32, LlmClientError> {
    if sources.is_empty() {
        tracing::debug!("judge.context_precision skipped: no sources");
        return Ok(0.0);
    }
    let prompt = build_context_precision_prompt(question, sources);
    tracing::debug!(
        question_len = question.len(),
        source_count = sources.len(),
        "judge.context_precision calling LLM"
    );
    let raw = judge
        .complete(&prompt, "")
        .instrument(tracing::debug_span!("judge.context_precision"))
        .await?;
    tracing::debug!(raw_response = %raw.trim(), "judge.context_precision raw response");
    let score = parse_faithfulness_score(&raw);
    if score.is_some() && raw.trim().parse::<f32>().is_err() {
        tracing::warn!(raw_response = %raw.trim(), "judge.context_precision: extracted score from noisy response");
    }
    score.ok_or_else(|| {
        LlmClientError::InvalidResponse(format!(
            "judge returned non-numeric precision score: '{}'",
            raw.trim()
        ))
    })
}

/// Chunk quality via LLM-as-judge for ingestion events.
/// The judge evaluates a sample of chunks for coherence, segmentation, readability,
/// and information density. Returns a score in `0.0..=1.0`.
pub async fn compute_chunk_quality(
    judge: &dyn LlmClient,
    filename: &str,
    content_type: &str,
    chunk_samples: &[EvalSource],
) -> Result<f32, LlmClientError> {
    if chunk_samples.is_empty() {
        tracing::debug!("judge.chunk_quality skipped: no chunk samples");
        return Ok(0.0);
    }
    let prompt = build_chunk_quality_prompt(filename, content_type, chunk_samples);
    tracing::debug!(
        filename,
        content_type,
        sample_count = chunk_samples.len(),
        "judge.chunk_quality calling LLM"
    );
    let raw = judge
        .complete(&prompt, "")
        .instrument(tracing::debug_span!("judge.chunk_quality"))
        .await?;
    tracing::debug!(raw_response = %raw.trim(), "judge.chunk_quality raw response");
    let score = parse_faithfulness_score(&raw);
    if score.is_some() && raw.trim().parse::<f32>().is_err() {
        tracing::warn!(raw_response = %raw.trim(), "judge.chunk_quality: extracted score from noisy response");
    }
    score.ok_or_else(|| {
        LlmClientError::InvalidResponse(format!(
            "judge returned non-numeric chunk quality score: '{}'",
            raw.trim()
        ))
    })
}

fn build_answer_relevancy_prompt(question: &str, generated_answer: &str) -> String {
    format!(
        "You are a strict relevancy evaluator.\n\
         Your task: Determine whether the ANSWER directly and completely addresses the QUESTION.\n\
         Do not evaluate factual correctness — only relevancy and completeness of coverage.\n\n\
         QUESTION:\n{question}\n\n\
         ANSWER:\n{answer}\n\n\
         Instructions:\n\
         - Score 1.0 if the answer directly and fully addresses the question.\n\
         - Score 0.0 if the answer is completely off-topic or refuses to answer.\n\
         - Use intermediate values for partial coverage (e.g. 0.5 if only half the question is addressed).\n\
         - Reply with ONLY a single decimal number between 0.0 and 1.0. No explanation. No other text.\n\n\
         Score:",
        question = question,
        answer = generated_answer
    )
}

fn build_context_precision_prompt(question: &str, sources: &[EvalSource]) -> String {
    let chunks = sources
        .iter()
        .enumerate()
        .map(|(i, s)| format!("Chunk {}:\n{}", i + 1, s.text))
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "You are a strict retrieval quality evaluator.\n\
         Your task: Determine what fraction of the RETRIEVED CHUNKS are actually relevant to answering the QUESTION.\n\
         Do not evaluate whether the answer is correct — only whether each chunk contains useful information for the question.\n\n\
         QUESTION:\n{question}\n\n\
         RETRIEVED CHUNKS:\n{chunks}\n\n\
         Instructions:\n\
         - Score 1.0 if all chunks are relevant to the question.\n\
         - Score 0.0 if none of the chunks are relevant.\n\
         - Use intermediate values proportional to the fraction of relevant chunks (e.g. 0.5 if half are relevant).\n\
         - Reply with ONLY a single decimal number between 0.0 and 1.0. No explanation. No other text.\n\n\
         Score:",
        question = question,
        chunks = chunks
    )
}

fn build_chunk_quality_prompt(
    filename: &str,
    content_type: &str,
    chunk_samples: &[EvalSource],
) -> String {
    let chunks = chunk_samples
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let page_info = s.page.map(|p| format!(" (page {p})")).unwrap_or_default();
            format!("Chunk {}{page_info}:\n{}", i + 1, s.text)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "You are a document ingestion quality evaluator.\n\
         A document \"{filename}\" (type: {content_type}) was split into chunks.\n\
         Below are {n} sample chunks. Evaluate the overall chunk quality.\n\n\
         CHUNK SAMPLES:\n{chunks}\n\n\
         Evaluate these dimensions:\n\
         - COHERENCE: Does each chunk contain complete, self-contained thoughts? \
           Or are sentences cut mid-way?\n\
         - SEGMENTATION: Are chunk boundaries at natural break points \
           (paragraphs, sections)?\n\
         - READABILITY: Is the text clean and well-formed? Or does it contain \
           garbled characters, OCR artifacts, or encoding issues?\n\
         - INFORMATION DENSITY: Do chunks contain meaningful content, \
           not just headers/footers/boilerplate?\n\n\
         Instructions:\n\
         - Score 1.0 if all chunks are coherent, well-segmented, readable, \
           and information-dense.\n\
         - Score 0.0 if chunks are garbled, incoherent, or mostly empty/boilerplate.\n\
         - Use intermediate values for partial quality.\n\
         - Reply with ONLY a single decimal number between 0.0 and 1.0. \
           No explanation. No other text.\n\n\
         Score:",
        filename = filename,
        content_type = content_type,
        n = chunk_samples.len(),
        chunks = chunks,
    )
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

/// Generates a concise 1-2 sentence human-readable description of an eval run.
/// Summarises what happened (operation type, tool calls) and what the scores mean.
/// Returns the trimmed LLM response directly — no score parsing.
#[allow(clippy::too_many_arguments)]
pub async fn generate_eval_description(
    judge: &dyn LlmClient,
    operation_type: &str,
    question: &str,
    generated_answer: &str,
    faithfulness: f32,
    answer_relevancy: Option<f32>,
    context_precision: Option<f32>,
    tool_call_count: usize,
) -> Result<String, LlmClientError> {
    let scores = {
        let mut parts = vec![format!("faithfulness={faithfulness:.2}")];
        if let Some(ar) = answer_relevancy {
            parts.push(format!("answer_relevancy={ar:.2}"));
        }
        if let Some(cp) = context_precision {
            parts.push(format!("context_precision={cp:.2}"));
        }
        parts.join(", ")
    };

    let prompt = format!(
        "You are an evaluation summariser. Write exactly 1-2 plain-English sentences (max 500 characters) \
         describing what was evaluated and what the scores indicate. Be specific and factual. \
         Do not include the raw numbers — integrate them naturally into the description.\n\n\
         Operation: {operation_type}\n\
         Question: {question}\n\
         Answer (first 300 chars): {answer_preview}\n\
         Tool calls made: {tool_call_count}\n\
         Scores: {scores}\n\n\
         Summary:",
        operation_type = operation_type,
        question = question,
        answer_preview = &generated_answer[..generated_answer.len().min(300)],
        tool_call_count = tool_call_count,
        scores = scores,
    );

    tracing::debug!(
        operation_type,
        tool_call_count,
        "generate_eval_description calling LLM"
    );
    let raw = judge
        .complete(&prompt, "")
        .instrument(tracing::debug_span!("judge.eval_description"))
        .await?;
    tracing::debug!(raw_response = %raw.trim(), "generate_eval_description raw response");

    Ok(raw.trim().to_string())
}

/// Parses the first valid f32 in `[0.0, 1.0]` found anywhere in `raw`.
/// Scans all whitespace-separated tokens so that responses like
/// `"Assistant: 0.5"` or `"Score: 0.85"` are handled correctly.
/// Returns `None` if no valid score is found.
pub(crate) fn parse_faithfulness_score(raw: &str) -> Option<f32> {
    raw.split_whitespace()
        .filter_map(|token| {
            // Strip common trailing punctuation (e.g. "0.5.")
            token.trim_end_matches('.').parse::<f32>().ok()
        })
        .find(|s| (0.0f32..=1.0f32).contains(s))
}
