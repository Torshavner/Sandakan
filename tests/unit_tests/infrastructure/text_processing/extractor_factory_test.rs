use sandakan::infrastructure::text_processing::{ExtractorFactory, ExtractorFactoryError};
use sandakan::presentation::ExtractorProvider;
use sandakan::presentation::config::PdfExtractionSettings;

fn azure_settings(endpoint: Option<&str>, key: Option<&str>) -> PdfExtractionSettings {
    PdfExtractionSettings {
        enabled: true,
        max_file_size_mb: 50,
        provider: ExtractorProvider::Azure,
        vlm_model: None,
        vlm_revision: None,
        vlm_base_url: None,
        vlm_api_key: None,
        azure_endpoint: endpoint.map(str::to_string),
        azure_key: key.map(str::to_string),
    }
}

fn lm_studio_settings(base_url: Option<&str>, model: Option<&str>) -> PdfExtractionSettings {
    PdfExtractionSettings {
        enabled: true,
        max_file_size_mb: 50,
        provider: ExtractorProvider::LmStudio,
        vlm_model: model.map(str::to_string),
        vlm_revision: None,
        vlm_base_url: base_url.map(str::to_string),
        vlm_api_key: Some("lm-studio".to_string()),
        azure_endpoint: None,
        azure_key: None,
    }
}

#[tokio::test]
async fn given_azure_provider_without_endpoint_when_creating_then_returns_missing_endpoint_error() {
    let settings = azure_settings(None, Some("sk-test-key"));

    let result = ExtractorFactory::create(&settings);

    assert!(matches!(
        result,
        Err(ExtractorFactoryError::MissingAzureEndpoint)
    ));
}

#[tokio::test]
async fn given_azure_provider_without_key_when_creating_then_returns_missing_key_error() {
    let settings = azure_settings(Some("https://example.cognitiveservices.azure.com"), None);

    let result = ExtractorFactory::create(&settings);

    assert!(matches!(
        result,
        Err(ExtractorFactoryError::MissingAzureKey)
    ));
}

#[tokio::test]
async fn given_azure_provider_with_valid_config_when_creating_then_returns_ok() {
    let settings = azure_settings(
        Some("https://example.cognitiveservices.azure.com"),
        Some("sk-test-key"),
    );

    let result = ExtractorFactory::create(&settings);

    assert!(result.is_ok());
}

#[tokio::test]
async fn given_lm_studio_provider_without_base_url_when_creating_then_returns_missing_url_error() {
    let settings = lm_studio_settings(None, Some("llava-phi-3-mini"));

    let result = ExtractorFactory::create(&settings);

    assert!(matches!(
        result,
        Err(ExtractorFactoryError::MissingVlmBaseUrl)
    ));
}

#[tokio::test]
async fn given_lm_studio_provider_without_model_when_creating_then_returns_missing_model_error() {
    let settings = lm_studio_settings(Some("http://localhost:1234"), None);

    let result = ExtractorFactory::create(&settings);

    assert!(matches!(
        result,
        Err(ExtractorFactoryError::MissingVlmModel)
    ));
}

#[tokio::test]
async fn given_lm_studio_provider_with_valid_config_when_creating_then_returns_ok() {
    let settings = lm_studio_settings(
        Some("http://localhost:1234"),
        Some("xtuner/llava-phi-3-mini-gguf"),
    );

    let result = ExtractorFactory::create(&settings);

    assert!(result.is_ok());
}
