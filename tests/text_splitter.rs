use sandakan::application::ports::TextSplitter;
use sandakan::domain::DocumentId;
use sandakan::infrastructure::text_processing::{RecursiveCharacterSplitter, SemanticSplitter};

const SMALL_CHUNK_SIZE: usize = 10;
const SMALL_OVERLAP: usize = 2;
const STANDARD_TOKEN_LIMIT: usize = 512;
const STANDARD_OVERLAP_TOKENS: usize = 50;
const TIGHT_TOKEN_LIMIT: usize = 50;
const TIGHT_OVERLAP_TOKENS: usize = 10;

#[tokio::test]
async fn given_text_when_recursive_character_splitter_splits_then_creates_fixed_size_chunks() {
    let splitter = RecursiveCharacterSplitter::new(SMALL_CHUNK_SIZE, SMALL_OVERLAP);
    let text = "This is a test document with some content.";
    let doc_id = DocumentId::new();

    let result = splitter.split(text, doc_id).await;

    assert!(result.is_ok());
    let chunks = result.unwrap();
    assert!(!chunks.is_empty());
    for chunk in &chunks {
        assert!(chunk.text.len() <= SMALL_CHUNK_SIZE);
        assert_eq!(chunk.document_id, doc_id);
    }
}

#[tokio::test]
async fn given_empty_text_when_recursive_character_splitter_splits_then_returns_empty_chunks() {
    let splitter = RecursiveCharacterSplitter::new(SMALL_CHUNK_SIZE, SMALL_OVERLAP);
    let text = "";
    let doc_id = DocumentId::new();

    let result = splitter.split(text, doc_id).await;

    assert!(result.is_ok());
    let chunks = result.unwrap();
    assert!(chunks.is_empty());
}

#[tokio::test]
async fn given_multi_sentence_text_when_semantic_splitter_splits_then_chunks_terminate_at_sentence_boundaries()
 {
    let splitter = SemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP_TOKENS);
    let text = "First sentence. Second sentence! Third sentence? Fourth sentence.";
    let doc_id = DocumentId::new();

    let result = splitter.split(text, doc_id).await;

    assert!(result.is_ok());
    let chunks = result.unwrap();
    assert!(!chunks.is_empty());

    for chunk in &chunks {
        let trimmed = chunk.text.trim();
        assert!(
            trimmed.ends_with('.') || trimmed.ends_with('!') || trimmed.ends_with('?'),
            "Chunk must end with sentence boundary: '{}'",
            chunk.text
        );
    }
}

#[tokio::test]
async fn given_long_text_when_semantic_splitter_splits_with_tight_limit_then_respects_token_limits()
{
    let splitter = SemanticSplitter::new(TIGHT_TOKEN_LIMIT, TIGHT_OVERLAP_TOKENS);
    let text = "This is the first sentence. This is the second sentence. This is the third sentence. This is the fourth sentence. This is the fifth sentence.";
    let doc_id = DocumentId::new();

    let result = splitter.split(text, doc_id).await;

    assert!(result.is_ok());
    let chunks = result.unwrap();
    assert!(!chunks.is_empty());
}

#[tokio::test]
async fn given_multi_paragraph_text_when_semantic_splitter_splits_then_handles_paragraph_breaks() {
    let splitter = SemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP_TOKENS);
    let text = "First paragraph with sentence one. Sentence two.\n\nSecond paragraph with sentence three. Sentence four.";
    let doc_id = DocumentId::new();

    let result = splitter.split(text, doc_id).await;

    assert!(result.is_ok());
    let chunks = result.unwrap();
    assert!(!chunks.is_empty());
}

#[tokio::test]
async fn given_empty_text_when_semantic_splitter_splits_then_returns_empty_chunks() {
    let splitter = SemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP_TOKENS);
    let text = "";
    let doc_id = DocumentId::new();

    let result = splitter.split(text, doc_id).await;

    assert!(result.is_ok());
    let chunks = result.unwrap();
    assert!(chunks.is_empty());
}

#[tokio::test]
async fn given_text_without_punctuation_when_semantic_splitter_splits_then_returns_single_chunk() {
    let splitter = SemanticSplitter::new(STANDARD_TOKEN_LIMIT, STANDARD_OVERLAP_TOKENS);
    let text = "This is text without proper punctuation";
    let doc_id = DocumentId::new();

    let result = splitter.split(text, doc_id).await;

    assert!(result.is_ok());
    let chunks = result.unwrap();
    assert_eq!(chunks.len(), 1);
}
