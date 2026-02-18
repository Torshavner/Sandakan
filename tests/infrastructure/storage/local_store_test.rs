use std::io;

use bytes::Bytes;
use futures::stream;

use sandakan::application::ports::StagingStore;
use sandakan::domain::{DocumentId, StoragePath};
use sandakan::infrastructure::storage::LocalStagingStore;

fn create_test_store() -> (tempfile::TempDir, LocalStagingStore) {
    let dir = tempfile::TempDir::new().unwrap();
    let store = LocalStagingStore::new(dir.path().to_path_buf()).unwrap();
    (dir, store)
}

#[tokio::test]
async fn given_valid_stream_when_storing_then_file_is_persisted() {
    let (_dir, store) = create_test_store();
    let doc_id = DocumentId::new();
    let path = StoragePath::new(&doc_id, "test.txt");

    let chunks = vec![Ok(Bytes::from("hello ")), Ok(Bytes::from("world"))];
    let byte_stream = Box::pin(stream::iter(chunks));

    let size = store.store(&path, byte_stream, None).await.unwrap();
    assert_eq!(size, 11);
}

#[tokio::test]
async fn given_stored_file_when_fetching_then_bytes_match_original() {
    let (_dir, store) = create_test_store();
    let doc_id = DocumentId::new();
    let path = StoragePath::new(&doc_id, "test.txt");

    let content = b"test content";
    let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from(&content[..]))]));
    store.store(&path, byte_stream, None).await.unwrap();

    let fetched = store.fetch(&path).await.unwrap();
    assert_eq!(fetched, content);
}

#[tokio::test]
async fn given_stored_file_when_deleting_then_fetch_returns_not_found() {
    let (_dir, store) = create_test_store();
    let doc_id = DocumentId::new();
    let path = StoragePath::new(&doc_id, "test.txt");

    let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from("data"))]));
    store.store(&path, byte_stream, None).await.unwrap();

    store.delete(&path).await.unwrap();

    let result = store.fetch(&path).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn given_stream_error_when_storing_then_returns_error() {
    let (_dir, store) = create_test_store();
    let doc_id = DocumentId::new();
    let path = StoragePath::new(&doc_id, "test.txt");

    let chunks: Vec<Result<Bytes, io::Error>> = vec![
        Ok(Bytes::from("partial")),
        Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "network drop",
        )),
    ];
    let byte_stream = Box::pin(stream::iter(chunks));

    let result = store.store(&path, byte_stream, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn given_nonexistent_path_when_fetching_then_returns_not_found() {
    let (_dir, store) = create_test_store();
    let doc_id = DocumentId::new();
    let path = StoragePath::new(&doc_id, "nonexistent.txt");

    let result = store.fetch(&path).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn given_stored_file_when_head_then_returns_size() {
    let (_dir, store) = create_test_store();
    let doc_id = DocumentId::new();
    let path = StoragePath::new(&doc_id, "test.txt");

    let content = b"hello world";
    let byte_stream = Box::pin(stream::iter(vec![Ok(Bytes::from(&content[..]))]));
    store.store(&path, byte_stream, None).await.unwrap();

    let size = store.head(&path).await.unwrap();
    assert_eq!(size, 11);
}

#[tokio::test]
async fn given_nonexistent_path_when_head_then_returns_error() {
    let (_dir, store) = create_test_store();
    let doc_id = DocumentId::new();
    let path = StoragePath::new(&doc_id, "nonexistent.txt");

    let result = store.head(&path).await;
    assert!(result.is_err());
}
