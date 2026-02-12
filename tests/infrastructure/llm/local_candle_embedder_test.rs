use sandakan::application::ports::Embedder;
use sandakan::infrastructure::llm::LocalCandleEmbedder;

#[test]
fn given_valid_model_id_when_creating_embedder_then_loads_successfully() {
    let result = LocalCandleEmbedder::new("sentence-transformers/all-MiniLM-L6-v2");
    assert!(result.is_ok());
}

#[test]
fn given_invalid_model_id_when_creating_embedder_then_returns_error() {
    let result = LocalCandleEmbedder::new("nonexistent/model-that-does-not-exist");
    assert!(result.is_err());
}

#[tokio::test]
async fn given_local_model_when_embedding_text_then_returns_384_dimensions() {
    let embedder = LocalCandleEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
        .expect("Failed to load model");

    let embedding = embedder
        .embed("Hello world")
        .await
        .expect("Failed to embed");

    assert_eq!(embedding.dimensions(), 384);
}

#[tokio::test]
async fn given_local_model_when_embedding_batch_then_returns_matching_count() {
    let embedder = LocalCandleEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
        .expect("Failed to load model");

    let texts = &["Hello world", "Rust is great", "Vector embeddings"];
    let embeddings = embedder
        .embed_batch(texts)
        .await
        .expect("Failed to embed batch");

    assert_eq!(embeddings.len(), 3);
    for embedding in &embeddings {
        assert_eq!(embedding.dimensions(), 384);
    }
}

#[tokio::test]
async fn given_similar_texts_when_embedded_then_cosine_similarity_is_high() {
    let embedder = LocalCandleEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
        .expect("Failed to load model");

    let emb_a = embedder
        .embed("The cat sat on the mat")
        .await
        .expect("Failed to embed");
    let emb_b = embedder
        .embed("A cat was sitting on a mat")
        .await
        .expect("Failed to embed");

    let similarity = emb_a.cosine_similarity(&emb_b);
    assert!(
        similarity > 0.7,
        "Expected high similarity for similar sentences, got {}",
        similarity
    );
}

#[tokio::test]
async fn given_dissimilar_texts_when_embedded_then_cosine_similarity_is_low() {
    let embedder = LocalCandleEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
        .expect("Failed to load model");

    let emb_a = embedder
        .embed("The cat sat on the mat")
        .await
        .expect("Failed to embed");
    let emb_b = embedder
        .embed("Quantum computing uses qubits for parallel calculations")
        .await
        .expect("Failed to embed");

    let similarity = emb_a.cosine_similarity(&emb_b);
    assert!(
        similarity < 0.5,
        "Expected low similarity for dissimilar sentences, got {}",
        similarity
    );
}

#[tokio::test]
async fn given_empty_batch_when_embedding_then_returns_empty_vec() {
    let embedder = LocalCandleEmbedder::new("sentence-transformers/all-MiniLM-L6-v2")
        .expect("Failed to load model");

    let texts: &[&str] = &[];
    let embeddings = embedder
        .embed_batch(texts)
        .await
        .expect("Failed to embed empty batch");

    assert!(embeddings.is_empty());
}
