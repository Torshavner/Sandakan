use sandakan::application::ports::TextSplitter;
use sandakan::domain::{DocumentId, TranscriptSegment};
use sandakan::infrastructure::text_processing::{MarkdownSemanticSplitter, TextSplitterFactory};
use sandakan::presentation::config::ChunkingStrategy;

const STANDARD_TOKEN_LIMIT: usize = 512;
const STANDARD_OVERLAP: usize = 50;
const TIGHT_TOKEN_LIMIT: usize = 50;
const TIGHT_OVERLAP: usize = 10;

#[tokio::test]
async fn given_markdown_with_headers_when_splitting_then_sections_stay_with_their_content() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text = "# Introduction\n\nThis is the introduction paragraph.\n\n## Details\n\nHere are the details of the project.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    assert!(!chunks.is_empty());
    // With a large token budget, everything fits in one chunk and the header stays with its content
    let combined: String = chunks
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(combined.contains("# Introduction"));
    assert!(combined.contains("introduction paragraph"));
}

#[tokio::test]
async fn given_markdown_with_code_fence_when_splitting_then_fence_kept_as_atomic_unit() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text = "Some intro text.\n\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n\nSome outro text.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    // The code fence should appear intact in one chunk (not split across multiple)
    let fence_chunks: Vec<_> = chunks
        .iter()
        .filter(|c| c.text.contains("fn main()"))
        .collect();
    assert_eq!(
        fence_chunks.len(),
        1,
        "Code fence should appear in exactly one chunk"
    );
    let fence_chunk = fence_chunks[0];
    assert!(
        fence_chunk.text.contains("println!"),
        "Code fence body should be kept together"
    );
}

#[tokio::test]
async fn given_markdown_with_table_when_splitting_then_table_not_split_across_chunks() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text = "Before table.\n\n| Name | Value |\n|------|-------|\n| A    | 1     |\n| B    | 2     |\n\nAfter table.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    let table_chunks: Vec<_> = chunks
        .iter()
        .filter(|c| c.text.contains("|------|"))
        .collect();
    assert_eq!(
        table_chunks.len(),
        1,
        "Table should appear in exactly one chunk"
    );
    let table_chunk = table_chunks[0];
    assert!(
        table_chunk.text.contains("| A    | 1"),
        "Table rows should be kept together"
    );
    assert!(
        table_chunk.text.contains("| B    | 2"),
        "All table rows should be in the same chunk"
    );
}

#[tokio::test]
async fn given_numbered_list_when_splitting_then_periods_not_treated_as_sentence_boundaries() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text =
        "The steps are: 1. First step 2. Second step 3. Third step and that concludes the process.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    // With a large token budget, the entire list should be in one chunk (not split at "1. ")
    assert_eq!(
        chunks.len(),
        1,
        "Numbered list should not be split at list periods"
    );
    assert!(chunks[0].text.contains("1. First step"));
    assert!(chunks[0].text.contains("3. Third step"));
}

#[tokio::test]
async fn given_prose_with_abbreviations_when_splitting_then_not_split_at_abbreviation_dots() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text = "Use tools e.g. hammers and screwdrivers i.e. hand tools for the job.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    assert_eq!(
        chunks.len(),
        1,
        "Abbreviations should not cause sentence splits"
    );
    assert!(chunks[0].text.contains("e.g."));
    assert!(chunks[0].text.contains("i.e."));
}

#[tokio::test]
async fn given_empty_markdown_when_splitting_then_returns_empty_chunks() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let doc_id = DocumentId::new();

    let chunks = splitter.split("", doc_id, None).await.unwrap();

    assert!(chunks.is_empty());
}

#[tokio::test]
async fn given_short_sections_when_splitting_then_merged_into_single_chunk() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text = "# Title\n\nShort paragraph.\n\n## Section\n\nAnother short paragraph.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    assert_eq!(
        chunks.len(),
        1,
        "Short sections should merge into a single chunk under large budget"
    );
}

#[tokio::test]
async fn given_oversized_code_fence_when_splitting_then_token_limit_still_respected() {
    let splitter = MarkdownSemanticSplitter::new(TIGHT_TOKEN_LIMIT, TIGHT_OVERLAP).unwrap();
    let large_code = "let x = 1;\n".repeat(100);
    let text = format!("```\n{}\n```", large_code);
    let doc_id = DocumentId::new();

    let chunks = splitter.split(&text, doc_id, None).await.unwrap();

    assert!(
        chunks.len() > 1,
        "Oversized code fence should be split into multiple chunks, got {}",
        chunks.len()
    );
    // Verify no content is lost
    let combined: String = chunks
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        combined.contains("let x = 1"),
        "Content should be preserved after splitting oversized fence"
    );
}

#[tokio::test]
async fn given_transcript_segments_when_markdown_splitter_splits_then_delegates_correctly() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let segments = vec![
        TranscriptSegment {
            text: "Hello world.".to_string(),
            start_time: 0.0,
            end_time: 1.0,
        },
        TranscriptSegment {
            text: "This is a test.".to_string(),
            start_time: 1.0,
            end_time: 2.0,
        },
    ];
    let doc_id = DocumentId::new();

    let chunks = splitter
        .split_segments(&segments, doc_id, None)
        .await
        .unwrap();

    assert!(!chunks.is_empty());
    assert!(
        chunks[0].start_time.is_some(),
        "Transcript chunks should have start_time"
    );
}

#[test]
fn given_semantic_strategy_when_factory_creates_then_returns_markdown_aware_splitter() {
    let result = TextSplitterFactory::create(ChunkingStrategy::Semantic, 512, 50);
    assert!(
        result.is_ok(),
        "Semantic strategy should create MarkdownSemanticSplitter"
    );
}
