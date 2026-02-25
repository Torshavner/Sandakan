use std::sync::Arc;

use sandakan::domain::{Chunk, Document};
use sandakan::domain::{ContentType, DocumentId, DocumentMetadata};

fn make_document(filename: &str) -> Document {
    Document::new(filename.to_string(), ContentType::Pdf, 1024)
}

// ─── DocumentMetadata construction ───────────────────────────────────────────

#[test]
fn given_document_with_extension_when_creating_metadata_then_strips_extension_for_title() {
    let doc = make_document("annual_report.pdf");

    let meta = DocumentMetadata::from_document(&doc, None);

    assert_eq!(meta.title, "annual_report");
}

#[test]
fn given_document_without_extension_when_creating_metadata_then_uses_full_filename() {
    let doc = make_document("report");

    let meta = DocumentMetadata::from_document(&doc, None);

    assert_eq!(meta.title, "report");
}

#[test]
fn given_document_with_source_url_when_creating_metadata_then_preserves_source_url() {
    let doc = make_document("slides.pdf");
    let url = Some("https://example.com/slides.pdf".to_string());

    let meta = DocumentMetadata::from_document(&doc, url.clone());

    assert_eq!(meta.source_url, url);
}

#[test]
fn given_document_with_no_source_url_when_creating_metadata_then_source_url_is_none() {
    let doc = make_document("notes.pdf");

    let meta = DocumentMetadata::from_document(&doc, None);

    assert!(meta.source_url.is_none());
}

// ─── Chunk::as_contextual_string ─────────────────────────────────────────────

#[test]
fn given_chunk_with_metadata_and_page_when_contextual_string_then_includes_title_and_page() {
    let doc = make_document("lecture_notes.pdf");
    let meta = Arc::new(DocumentMetadata::from_document(&doc, None));
    let chunk = Chunk::with_metadata(
        "Some important content.".to_string(),
        DocumentId::new(),
        Some(3),
        0,
        meta,
    );

    let result = chunk.as_contextual_string();

    assert!(
        result.contains("Title: lecture_notes"),
        "expected title in: {result}"
    );
    assert!(result.contains("Page: 3"), "expected page in: {result}");
    assert!(
        result.contains("Content: Some important content."),
        "expected content in: {result}"
    );
}

#[test]
fn given_chunk_without_metadata_when_contextual_string_then_returns_raw_text() {
    let chunk = Chunk::new(
        "Plain text content.".to_string(),
        DocumentId::new(),
        Some(1),
        0,
    );

    let result = chunk.as_contextual_string();

    assert_eq!(result, "Plain text content.");
}

#[test]
fn given_chunk_with_no_page_when_contextual_string_then_shows_na_for_page() {
    let doc = make_document("transcript.mp4");
    let meta = Arc::new(DocumentMetadata::from_document(&doc, None));
    let chunk = Chunk::with_metadata(
        "Spoken word content.".to_string(),
        DocumentId::new(),
        None,
        0,
        meta,
    );

    let result = chunk.as_contextual_string();

    assert!(
        result.contains("Page: N/A"),
        "expected N/A page in: {result}"
    );
}

#[test]
fn given_two_chunks_from_same_doc_when_created_with_metadata_then_share_same_arc() {
    let doc = make_document("shared_doc.pdf");
    let meta = Arc::new(DocumentMetadata::from_document(&doc, None));
    let doc_id = DocumentId::new();

    let chunk_a = Chunk::with_metadata(
        "First chunk.".to_string(),
        doc_id,
        Some(1),
        0,
        Arc::clone(&meta),
    );
    let chunk_b = Chunk::with_metadata(
        "Second chunk.".to_string(),
        doc_id,
        Some(1),
        12,
        Arc::clone(&meta),
    );

    let meta_a = chunk_a.metadata.as_ref().unwrap();
    let meta_b = chunk_b.metadata.as_ref().unwrap();
    assert!(
        Arc::ptr_eq(meta_a, meta_b),
        "both chunks should reference the same Arc"
    );
}

// ─── Splitter metadata propagation ───────────────────────────────────────────

#[tokio::test]
async fn given_metadata_when_splitting_text_then_all_chunks_carry_metadata() {
    use sandakan::application::ports::TextSplitter;
    use sandakan::infrastructure::text_processing::RecursiveCharacterSplitter;

    let splitter = RecursiveCharacterSplitter::new(50, 5);
    let doc = make_document("my_document.pdf");
    let meta = Arc::new(DocumentMetadata::from_document(&doc, None));
    let doc_id = DocumentId::new();

    let chunks = splitter
        .split(
            "Hello world. This is test content for splitting.",
            doc_id,
            Some(Arc::clone(&meta)),
        )
        .await
        .unwrap();

    assert!(!chunks.is_empty());
    for chunk in &chunks {
        let chunk_meta = chunk.metadata.as_ref().expect("chunk should have metadata");
        assert_eq!(chunk_meta.title, "my_document");
    }
}
