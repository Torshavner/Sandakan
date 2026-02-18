use sandakan::infrastructure::llm::EmbedderFactory;
use sandakan::presentation::config::EmbeddingProvider;

#[test]
fn given_openai_provider_with_key_when_creating_then_succeeds() {
    let result = EmbedderFactory::create(
        EmbeddingProvider::OpenAi,
        "text-embedding-3-small".to_string(),
        Some("sk-test-key".to_string()),
    );

    assert!(result.is_ok());
}

#[test]
fn given_openai_provider_without_key_when_creating_then_returns_error() {
    let result = EmbedderFactory::create(
        EmbeddingProvider::OpenAi,
        "text-embedding-3-small".to_string(),
        None,
    );

    assert!(result.is_err());
}

#[test]
fn given_openai_provider_with_empty_key_when_creating_then_returns_error() {
    let result = EmbedderFactory::create(
        EmbeddingProvider::OpenAi,
        "text-embedding-3-small".to_string(),
        Some(String::new()),
    );

    assert!(result.is_err());
}
