use sandakan::domain::SparseEmbedding;

#[test]
fn given_unsorted_pairs_when_creating_sparse_embedding_then_indices_sorted_ascending() {
    let embedding = SparseEmbedding::new(vec![(30, 0.5), (10, 0.3), (20, 0.7)]);

    assert_eq!(embedding.indices, vec![10, 20, 30]);
    assert_eq!(embedding.values, vec![0.3, 0.7, 0.5]);
}

#[test]
fn given_duplicate_indices_when_creating_sparse_embedding_then_duplicates_removed() {
    let embedding = SparseEmbedding::new(vec![(10, 0.3), (10, 0.9), (20, 0.5)]);

    assert_eq!(embedding.indices, vec![10, 20]);
    assert_eq!(embedding.len(), 2);
}

#[test]
fn given_empty_pairs_when_creating_sparse_embedding_then_is_empty_returns_true() {
    let embedding = SparseEmbedding::new(vec![]);

    assert!(embedding.is_empty());
    assert_eq!(embedding.len(), 0);
}

#[test]
fn given_non_empty_pairs_when_checking_is_empty_then_returns_false() {
    let embedding = SparseEmbedding::new(vec![(1, 0.5)]);

    assert!(!embedding.is_empty());
    assert_eq!(embedding.len(), 1);
}

#[test]
fn given_single_pair_when_creating_sparse_embedding_then_preserves_values() {
    let embedding = SparseEmbedding::new(vec![(42, 0.99)]);

    assert_eq!(embedding.indices, vec![42]);
    assert_eq!(embedding.values, vec![0.99]);
}
