use sandakan::domain::Embedding;

#[test]
fn given_embedding_when_checking_dimensions_then_returns_correct_size() {
    let embedding = Embedding::new(vec![0.1, 0.2, 0.3]);
    assert_eq!(embedding.dimensions(), 3);
}

#[test]
fn given_identical_vectors_when_computing_similarity_then_returns_one() {
    let a = Embedding::new(vec![1.0, 0.0, 0.0]);
    let b = Embedding::new(vec![1.0, 0.0, 0.0]);

    let similarity = a.cosine_similarity(&b);
    assert!((similarity - 1.0).abs() < 0.001);
}

#[test]
fn given_orthogonal_vectors_when_computing_similarity_then_returns_zero() {
    let a = Embedding::new(vec![1.0, 0.0, 0.0]);
    let b = Embedding::new(vec![0.0, 1.0, 0.0]);

    let similarity = a.cosine_similarity(&b);
    assert!(similarity.abs() < 0.001);
}
