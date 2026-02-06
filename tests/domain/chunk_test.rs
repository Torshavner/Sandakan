use sandakan::domain::{Chunk, ChunkId, DocumentId};

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
}
