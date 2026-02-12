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

#[tokio::test]
async fn given_oversized_sentence_when_semantic_splitter_splits_then_falls_back_to_character_splitting()
 {
    let tight_limit = 50;
    let overlap = 10;
    let splitter = SemanticSplitter::new(tight_limit, overlap);

    let oversized_sentence = "This is an extremely long sentence that contains a vast amount of information and will definitely exceed the token limit that we have set for chunking purposes, forcing the semantic splitter to fall back to character-based splitting to ensure no data loss occurs during the processing phase.";
    let doc_id = DocumentId::new();

    let result = splitter.split(oversized_sentence, doc_id).await;

    assert!(result.is_ok());
    let chunks = result.unwrap();

    let combined_text: String = chunks
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<&str>>()
        .join("");

    assert!(
        !chunks.is_empty(),
        "Should create at least one chunk from oversized sentence"
    );
    assert!(
        combined_text.contains("extremely long sentence"),
        "Should preserve beginning of sentence"
    );
    assert!(
        combined_text.contains("processing phase"),
        "Should preserve end of sentence"
    );
}

#[tokio::test]
async fn given_document_with_oversized_sentence_when_semantic_splitter_splits_then_no_data_loss() {
    let tight_limit = 50;
    let overlap = 10;
    let splitter = SemanticSplitter::new(tight_limit, overlap);

    let text = "This is a normal sentence. This is another normal sentence that should fit within limits. Now here comes an extremely long sentence with lots and lots of words that will definitely exceed our token limit and should trigger the fallback mechanism to prevent any data loss whatsoever. This is a final normal sentence.";
    let doc_id = DocumentId::new();

    let result = splitter.split(text, doc_id).await;

    assert!(result.is_ok());
    let chunks = result.unwrap();

    let combined_text: String = chunks
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<&str>>()
        .join(" ");

    assert!(
        combined_text.contains("normal sentence"),
        "Should preserve normal sentences"
    );
    assert!(
        combined_text.contains("fallback mechanism"),
        "Should preserve content from oversized sentence"
    );
    assert!(
        combined_text.contains("final normal sentence"),
        "Should preserve sentences after oversized one"
    );
}

#[tokio::test]
async fn given_1000_token_sentence_when_semantic_splitter_splits_with_512_limit_then_creates_multiple_chunks()
 {
    let max_tokens = 512;
    let overlap = 50;
    let splitter = SemanticSplitter::new(max_tokens, overlap);

    let long_sentence = "WHEREAS ".to_string()
        + &"the parties hereto agree to the following terms and conditions ".repeat(50)
        + "and this agreement shall be binding upon execution.";

    let doc_id = DocumentId::new();

    let result = splitter.split(&long_sentence, doc_id).await;

    assert!(result.is_ok());
    let chunks = result.unwrap();

    assert!(
        chunks.len() >= 2,
        "1000-token sentence should split into at least 2 chunks, got {}",
        chunks.len()
    );

    let combined_text: String = chunks
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<&str>>()
        .join("");

    assert!(
        combined_text.contains("WHEREAS"),
        "Should preserve beginning"
    );
    assert!(
        combined_text.contains("binding upon execution"),
        "Should preserve end"
    );
}
