use sandakan::domain::ContentType;
use sandakan::domain::{Chunk, ChunkId, DocumentId, DocumentMetadata};
use std::sync::Arc;

#[test]
fn given_two_chunk_ids_when_generated_then_are_unique() {
    let id1 = ChunkId::new();
    let id2 = ChunkId::new();
    assert_ne!(id1, id2);
}

#[test]
fn given_valid_params_when_creating_chunk_then_assigns_new_id() {
    let doc_id = DocumentId::new();
    let chunk = Chunk::new("test content".to_string(), doc_id, Some(1), 0);

    assert_eq!(chunk.text, "test content");
    assert_eq!(chunk.document_id, doc_id);
    assert_eq!(chunk.page, Some(1));
    assert_eq!(chunk.offset, 0);
    assert_eq!(chunk.start_time, None);
}

#[test]
fn given_chunk_when_with_start_time_called_then_start_time_is_set() {
    let doc_id = DocumentId::new();
    let chunk = Chunk::new("video text".to_string(), doc_id, None, 0).with_start_time(45.5);

    assert_eq!(chunk.start_time, Some(45.5));
}

#[test]
fn given_media_chunk_with_start_time_when_as_contextual_string_then_shows_seconds_label() {
    let doc_id = DocumentId::new();
    let meta = Arc::new(DocumentMetadata {
        title: "Lecture 1".to_string(),
        content_type: ContentType::Video,
        source_url: None,
    });
    let chunk = Chunk::with_metadata(
        "Neural networks explained.".to_string(),
        doc_id,
        None,
        0,
        meta,
    )
    .with_start_time(120.0);

    let ctx = chunk.as_contextual_string();
    assert!(ctx.contains("Title: Lecture 1"));
    assert!(ctx.contains("120.0s"));
    assert!(ctx.contains("Neural networks explained."));
}

#[test]
fn given_pdf_chunk_with_page_when_as_contextual_string_then_shows_page_number() {
    let doc_id = DocumentId::new();
    let meta = Arc::new(DocumentMetadata {
        title: "Report".to_string(),
        content_type: ContentType::Pdf,
        source_url: None,
    });
    let chunk = Chunk::with_metadata("Some PDF text.".to_string(), doc_id, Some(5), 0, meta);

    let ctx = chunk.as_contextual_string();
    assert!(ctx.contains("Page: 5"));
    assert!(!ctx.contains("s")); // no seconds label
}
