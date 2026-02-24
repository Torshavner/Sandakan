use sandakan::application::ports::{
    AgentMessage, Embedder, EmbedderError, LlmClient, LlmClientError, LlmToolResponse, ToolSchema,
};
use sandakan::application::services::eval_metrics::{
    compute_context_recall, compute_correctness, compute_faithfulness,
};
use sandakan::domain::{Embedding, EvalSource};

// -- Helpers ------------------------------------------------------------------

fn make_source(page: Option<u32>) -> EvalSource {
    EvalSource {
        text: "chunk".to_string(),
        page,
        score: 0.9,
    }
}

// -- Mocks --------------------------------------------------------------------

struct MockEmbedder;

#[async_trait::async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, _text: &str) -> Result<Embedding, EmbedderError> {
        Ok(Embedding::new(vec![0.1; 384]))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>, EmbedderError> {
        Ok(texts
            .iter()
            .map(|_| Embedding::new(vec![0.1; 384]))
            .collect())
    }
}

struct MockJudge {
    response: String,
}

#[async_trait::async_trait]
impl LlmClient for MockJudge {
    async fn complete(&self, _prompt: &str, _context: &str) -> Result<String, LlmClientError> {
        Ok(self.response.clone())
    }

    async fn complete_stream(
        &self,
        _prompt: &str,
        _context: &str,
    ) -> Result<
        std::pin::Pin<
            Box<
                dyn futures::stream::Stream<Item = Result<String, LlmClientError>> + Send + 'static,
            >,
        >,
        LlmClientError,
    > {
        Ok(Box::pin(futures::stream::once(async {
            Ok("0.9".to_string())
        })))
    }

    async fn complete_with_tools(
        &self,
        _messages: &[AgentMessage],
        _tools: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        unimplemented!()
    }
}

// -- Context Recall -----------------------------------------------------------

#[test]
fn given_all_expected_pages_found_when_computing_recall_then_returns_one() {
    let sources = vec![make_source(Some(3)), make_source(Some(5))];
    let recall = compute_context_recall(Some(&[3, 5]), &sources);
    assert!((recall - 1.0).abs() < f32::EPSILON);
}

#[test]
fn given_partial_pages_found_when_computing_recall_then_returns_fraction() {
    let sources = vec![make_source(Some(3)), make_source(Some(5))];
    let recall = compute_context_recall(Some(&[3, 5, 7]), &sources);
    let expected = 2.0f32 / 3.0;
    assert!((recall - expected).abs() < 0.001);
}

#[test]
fn given_no_expected_pages_when_computing_recall_then_returns_one() {
    let sources = vec![make_source(Some(1))];
    let recall = compute_context_recall(None, &sources);
    assert!((recall - 1.0).abs() < f32::EPSILON);
}

#[test]
fn given_empty_expected_pages_when_computing_recall_then_returns_one() {
    let sources = vec![make_source(Some(1))];
    let recall = compute_context_recall(Some(&[]), &sources);
    assert!((recall - 1.0).abs() < f32::EPSILON);
}

#[test]
fn given_no_pages_in_retrieved_chunks_when_computing_recall_then_returns_zero() {
    let sources = vec![make_source(None), make_source(None)];
    let recall = compute_context_recall(Some(&[3, 5]), &sources);
    assert!((recall - 0.0).abs() < f32::EPSILON);
}

// -- Answer Correctness -------------------------------------------------------

#[tokio::test]
async fn given_identical_answers_when_computing_correctness_then_score_is_near_one() {
    let embedder = MockEmbedder;
    let score = compute_correctness(&embedder, "same answer", "same answer")
        .await
        .unwrap();
    // Both embeddings are [0.1; 384] → cosine similarity = 1.0
    assert!((score - 1.0).abs() < 0.001);
}

// -- Faithfulness -------------------------------------------------------------

#[tokio::test]
async fn given_judge_returns_numeric_score_when_computing_faithfulness_then_extracts_float() {
    let judge = MockJudge {
        response: "0.85\n".to_string(),
    };
    let score = compute_faithfulness(&judge, "The answer.", "Context text.")
        .await
        .unwrap();
    assert!((score - 0.85).abs() < 0.001);
}

#[tokio::test]
async fn given_judge_returns_non_numeric_when_computing_faithfulness_then_returns_error() {
    let judge = MockJudge {
        response: "The answer is grounded.".to_string(),
    };
    let result = compute_faithfulness(&judge, "The answer.", "Context.").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn given_judge_returns_out_of_range_score_when_computing_faithfulness_then_returns_error() {
    let judge = MockJudge {
        response: "1.5".to_string(),
    };
    let result = compute_faithfulness(&judge, "The answer.", "Context.").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn given_judge_returns_zero_when_computing_faithfulness_then_score_is_zero() {
    let judge = MockJudge {
        response: "0.0".to_string(),
    };
    let score = compute_faithfulness(&judge, "Wrong answer.", "Context.")
        .await
        .unwrap();
    assert!((score - 0.0).abs() < f32::EPSILON);
}
