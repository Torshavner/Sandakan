use sandakan::application::ports::SparseEmbedder;
use sandakan::infrastructure::text_processing::Bm25SparseEmbedder;

#[tokio::test]
async fn given_normal_text_when_embedding_sparse_then_returns_non_empty_result() {
    let embedder = Bm25SparseEmbedder::new();

    let result = embedder
        .embed_sparse("machine learning models")
        .await
        .unwrap();

    assert!(!result.is_empty());
    assert!(result.values.iter().all(|v| *v > 0.0 && *v <= 1.0));
}

#[tokio::test]
async fn given_only_stop_words_when_embedding_sparse_then_returns_empty() {
    let embedder = Bm25SparseEmbedder::new();

    let result = embedder.embed_sparse("the is a an to for").await.unwrap();

    assert!(result.is_empty());
}

#[tokio::test]
async fn given_repeated_token_when_embedding_sparse_then_weight_increases() {
    let embedder = Bm25SparseEmbedder::new();

    let single = embedder.embed_sparse("rust").await.unwrap();
    let repeated = embedder.embed_sparse("rust rust rust other").await.unwrap();

    assert_eq!(single.len(), 1);
    assert!((single.values[0] - 1.0).abs() < f32::EPSILON);

    let rust_idx = single.indices[0];
    let pos = repeated
        .indices
        .iter()
        .position(|i| *i == rust_idx)
        .unwrap();
    assert!(repeated.values[pos] > 0.5);
}

#[tokio::test]
async fn given_batch_input_when_embedding_sparse_batch_then_returns_matching_count() {
    let embedder = Bm25SparseEmbedder::new();
    let texts = vec!["hello world", "rust programming", "vector search"];

    let results = embedder.embed_sparse_batch(&texts).await.unwrap();

    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|r| !r.is_empty()));
}

#[tokio::test]
async fn given_empty_text_when_embedding_sparse_then_returns_empty() {
    let embedder = Bm25SparseEmbedder::new();

    let result = embedder.embed_sparse("").await.unwrap();

    assert!(result.is_empty());
}

#[tokio::test]
async fn given_indices_when_embedding_sparse_then_indices_are_sorted() {
    let embedder = Bm25SparseEmbedder::new();

    let result = embedder
        .embed_sparse("alpha beta gamma delta epsilon zeta")
        .await
        .unwrap();

    let sorted: Vec<u32> = result.indices.clone();
    let mut check = sorted.clone();
    check.sort();
    assert_eq!(sorted, check);
}
