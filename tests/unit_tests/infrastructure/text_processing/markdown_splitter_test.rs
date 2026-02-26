// @AI-BYPASS-LENGTH
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
async fn given_short_sections_when_splitting_then_each_section_gets_own_chunk() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text = "# Title\n\nShort paragraph.\n\n## Section\n\nAnother short paragraph.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    // V2: hard boundary on headers — each section gets its own chunk
    assert_eq!(
        chunks.len(),
        2,
        "Each header section should be a separate chunk due to hard header boundaries"
    );
    assert!(
        chunks[0].text.contains("# Title"),
        "First chunk should contain first header"
    );
    assert!(
        chunks[1].text.contains("## Section"),
        "Second chunk should contain second header"
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
fn given_semantic_strategy_when_factory_creates_then_returns_text_and_markdown_splitters() {
    let result = TextSplitterFactory::create(ChunkingStrategy::Semantic, 512, 50);
    assert!(
        result.is_ok(),
        "Semantic strategy should create paired splitters"
    );
}

// --- V2 tests: header propagation, hard boundaries, paragraph atomicity ---

#[tokio::test]
async fn given_header_followed_by_prose_when_splitting_then_header_always_starts_chunk() {
    let splitter = MarkdownSemanticSplitter::new(TIGHT_TOKEN_LIMIT, TIGHT_OVERLAP).unwrap();
    let text = "Some preamble text here.\n\n## My Header\n\nSome content under the header.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    // The header should start a chunk, never be orphaned at the end of a previous chunk
    let header_chunk = chunks
        .iter()
        .find(|c| c.text.contains("## My Header"))
        .expect("A chunk should contain the header");
    assert!(
        header_chunk.text.starts_with("## My Header"),
        "Header should be at the start of its chunk, got: {:?}",
        header_chunk.text
    );
}

#[tokio::test]
async fn given_long_section_when_splitting_then_continuation_chunks_get_header_prefix() {
    // Use a tight token limit so the section must span multiple chunks
    let splitter = MarkdownSemanticSplitter::new(TIGHT_TOKEN_LIMIT, TIGHT_OVERLAP).unwrap();
    // Build a section with enough prose to overflow the token budget
    let long_prose = (0..20)
        .map(|i| format!("Sentence number {} about the topic.", i))
        .collect::<Vec<_>>()
        .join(" ");
    let text = format!("## Milvus\n\n{}", long_prose);
    let doc_id = DocumentId::new();

    let chunks = splitter.split(&text, doc_id, None).await.unwrap();

    assert!(
        chunks.len() >= 2,
        "Long section should produce multiple chunks, got {}",
        chunks.len()
    );
    // First chunk starts with the header
    assert!(
        chunks[0].text.starts_with("## Milvus"),
        "First chunk should start with header"
    );
    // Continuation chunks should also start with the header (header propagation)
    for (i, chunk) in chunks.iter().enumerate().skip(1) {
        assert!(
            chunk.text.starts_with("## Milvus"),
            "Continuation chunk {} should start with propagated header, got: {:?}",
            i,
            &chunk.text[..chunk.text.len().min(50)]
        );
    }
}

#[tokio::test]
async fn given_paragraph_under_budget_when_splitting_then_paragraph_kept_atomic() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text = "## Section\n\nFirst sentence of the paragraph. Second sentence of the paragraph. Third sentence of the paragraph.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    // The paragraph fits within the budget, so it should not be sentence-split
    assert_eq!(
        chunks.len(),
        1,
        "Paragraph under budget should be kept atomic in one chunk"
    );
    assert!(chunks[0].text.contains("First sentence"));
    assert!(chunks[0].text.contains("Third sentence"));
}

#[tokio::test]
async fn given_two_sections_when_splitting_then_hard_boundary_between_them() {
    // Use a generous budget so both sections easily fit in one chunk token-wise
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text = "## Section A\n\nContent A.\n\n## Section B\n\nContent B.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    // Even though both sections fit in the token budget, a new header forces a new chunk
    assert_eq!(
        chunks.len(),
        2,
        "Two sections should produce two chunks due to hard header boundary"
    );
    assert!(
        chunks[0].text.contains("Section A") && chunks[0].text.contains("Content A"),
        "First chunk should contain Section A content"
    );
    assert!(
        chunks[1].text.contains("Section B") && chunks[1].text.contains("Content B"),
        "Second chunk should contain Section B content"
    );
}

#[tokio::test]
async fn given_preamble_without_header_when_splitting_then_preamble_is_own_chunk() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text =
        "This is preamble text without any header.\n\n## First Section\n\nSection content here.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    assert_eq!(
        chunks.len(),
        2,
        "Preamble and header section should be separate chunks"
    );
    assert!(
        chunks[0].text.contains("preamble text"),
        "First chunk should be the preamble"
    );
    assert!(
        chunks[1].text.starts_with("## First Section"),
        "Second chunk should start with the header"
    );
}

// --- V3 tests: hierarchical header path propagation ---

#[tokio::test]
async fn given_nested_headers_when_splitting_then_chunks_contain_ancestor_breadcrumb() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text = "# Top Level\n\nIntro.\n\n## Mid Level\n\nMid content.\n\n### Deep Level\n\nDeep content here.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    assert_eq!(
        chunks.len(),
        3,
        "Three header levels should produce three chunks"
    );
    assert!(
        chunks[0].text.starts_with("# Top Level"),
        "First chunk: just top-level header"
    );
    assert!(
        chunks[1].text.starts_with("# Top Level > ## Mid Level"),
        "Second chunk should have breadcrumb, got: {:?}",
        &chunks[1].text[..chunks[1].text.len().min(60)]
    );
    assert!(
        chunks[2]
            .text
            .starts_with("# Top Level > ## Mid Level > ### Deep Level"),
        "Third chunk should have full breadcrumb, got: {:?}",
        &chunks[2].text[..chunks[2].text.len().min(80)]
    );
}

#[tokio::test]
async fn given_sibling_sections_when_splitting_then_each_gets_correct_ancestor_path() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text = "# Parent\n\n## Child A\n\nContent A.\n\n## Child B\n\nContent B.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    // # Parent has no content blocks so it's merged forward via breadcrumb
    assert_eq!(chunks.len(), 2);
    assert!(
        chunks[0].text.starts_with("# Parent > ## Child A"),
        "Child A should inherit Parent, got: {:?}",
        &chunks[0].text[..chunks[0].text.len().min(60)]
    );
    assert!(
        chunks[1].text.starts_with("# Parent > ## Child B"),
        "Child B should inherit Parent (not Child A), got: {:?}",
        &chunks[1].text[..chunks[1].text.len().min(60)]
    );
}

#[tokio::test]
async fn given_level_reset_when_splitting_then_ancestor_stack_pops_correctly() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text =
        "# Root\n\n## Branch A\n\n### Leaf\n\nLeaf content.\n\n## Branch B\n\nBranch B content.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    let branch_b_chunk = chunks
        .iter()
        .find(|c| c.text.contains("Branch B content"))
        .expect("Should have a chunk with Branch B content");
    assert!(
        branch_b_chunk.text.starts_with("# Root > ## Branch B"),
        "Branch B should not include Leaf or Branch A in path, got: {:?}",
        &branch_b_chunk.text[..branch_b_chunk.text.len().min(60)]
    );
    assert!(
        !branch_b_chunk.text.contains("Branch A"),
        "Branch B breadcrumb must not contain sibling Branch A"
    );
}

#[tokio::test]
async fn given_deep_hierarchy_when_splitting_then_continuation_chunks_get_full_breadcrumb() {
    let splitter = MarkdownSemanticSplitter::new(TIGHT_TOKEN_LIMIT, TIGHT_OVERLAP).unwrap();
    let long_prose = (0..20)
        .map(|i| format!("Sentence number {} about deep topic.", i))
        .collect::<Vec<_>>()
        .join(" ");
    let text = format!("# Root\n\n## Branch\n\n### Leaf\n\n{}", long_prose);
    let doc_id = DocumentId::new();

    let chunks = splitter.split(&text, doc_id, None).await.unwrap();

    let leaf_chunks: Vec<_> = chunks
        .iter()
        .filter(|c| c.text.contains("# Root > ## Branch > ### Leaf"))
        .collect();
    assert!(
        leaf_chunks.len() >= 2,
        "Leaf section should overflow into multiple chunks with breadcrumb, got {}",
        leaf_chunks.len()
    );
    for (i, chunk) in leaf_chunks.iter().enumerate() {
        assert!(
            chunk.text.starts_with("# Root > ## Branch > ### Leaf"),
            "Leaf chunk {} should start with full breadcrumb, got: {:?}",
            i,
            &chunk.text[..chunk.text.len().min(80)]
        );
    }
}

#[tokio::test]
async fn given_header_only_sections_when_splitting_then_no_empty_chunks_produced() {
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    // Headers without body content followed by a section with content
    let text = "# Speaker\n\n## Bio\n\n### Early Career\n\nActual biography content here.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    // Should produce exactly one chunk — the header-only sections are merged forward
    assert_eq!(
        chunks.len(),
        1,
        "Header-only sections should not produce separate chunks, got {} chunks",
        chunks.len()
    );
    assert!(
        chunks[0]
            .text
            .starts_with("# Speaker > ## Bio > ### Early Career"),
        "The chunk should carry the full breadcrumb, got: {:?}",
        &chunks[0].text[..chunks[0].text.len().min(80)]
    );
    assert!(
        chunks[0].text.contains("biography content"),
        "The chunk should contain the actual content"
    );
}

#[tokio::test]
async fn given_mixed_empty_and_content_sections_when_splitting_then_only_content_sections_chunked()
{
    let splitter = MarkdownSemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP).unwrap();
    let text = "# Title\n\n## Empty Section\n\n## Content Section\n\nReal content here.\n\n## Another Empty\n\n### Deep Empty\n\n## Final Section\n\nFinal content.";
    let doc_id = DocumentId::new();

    let chunks = splitter.split(text, doc_id, None).await.unwrap();

    assert_eq!(
        chunks.len(),
        2,
        "Only sections with content should produce chunks, got {}",
        chunks.len()
    );
    assert!(
        chunks[0].text.starts_with("# Title > ## Content Section"),
        "First content chunk should carry breadcrumb, got: {:?}",
        &chunks[0].text[..chunks[0].text.len().min(60)]
    );
    assert!(
        chunks[1].text.starts_with("# Title > ## Final Section"),
        "Second content chunk should carry breadcrumb, got: {:?}",
        &chunks[1].text[..chunks[1].text.len().min(60)]
    );
}
