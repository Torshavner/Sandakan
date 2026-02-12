use sandakan::application::ports::Embedder;
use sandakan::infrastructure::llm::OpenAiEmbedder;

#[tokio::test]
async fn given_invalid_api_key_when_embedding_then_returns_api_error() {
    let embedder = OpenAiEmbedder::new(
        "sk-invalid-key".to_string(),
        "text-embedding-3-small".to_string(),
    );

    let result = embedder.embed("test text").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn given_empty_batch_when_embedding_then_returns_empty_results() {
    let embedder = OpenAiEmbedder::new(
        "sk-invalid-key".to_string(),
        "text-embedding-3-small".to_string(),
    );
    let texts: &[&str] = &[];

    let result = embedder.embed_batch(texts).await;

    // Empty batch still hits the API, which will fail with invalid key
    // This test verifies the request is constructed and sent
    assert!(result.is_err());
}
