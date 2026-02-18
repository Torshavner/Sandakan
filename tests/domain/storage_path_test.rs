use sandakan::domain::{DocumentId, StoragePath};

#[test]
fn given_document_id_and_filename_when_creating_path_then_format_is_uuid_slash_filename() {
    let doc_id = DocumentId::new();
    let path = StoragePath::new(&doc_id, "lecture.mp4");

    let expected = format!("{}/lecture.mp4", doc_id.as_uuid());
    assert_eq!(path.as_str(), expected);
}

#[test]
fn given_two_different_documents_when_creating_paths_then_paths_differ() {
    let id_a = DocumentId::new();
    let id_b = DocumentId::new();

    let path_a = StoragePath::new(&id_a, "file.pdf");
    let path_b = StoragePath::new(&id_b, "file.pdf");

    assert_ne!(path_a, path_b);
}

#[test]
fn given_storage_path_when_displayed_then_matches_as_str() {
    let doc_id = DocumentId::new();
    let path = StoragePath::new(&doc_id, "test.txt");

    assert_eq!(format!("{}", path), path.as_str());
}
