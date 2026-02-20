use sandakan::infrastructure::text_processing::LmStudioVlmPdfAdapter;

#[test]
fn given_valid_config_when_constructing_adapter_then_builds_without_panic() {
    let _adapter = LmStudioVlmPdfAdapter::new(
        "http://localhost:1234",
        "qwen2.5-vl-7b-instruct",
        "test-key",
    );
}
