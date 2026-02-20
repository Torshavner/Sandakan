use sandakan::infrastructure::text_processing::{MAX_PAGES_DUE_TO_RAM_USAGE, parse_shard_names};

#[tokio::test]
async fn given_page_count_above_limit_when_computing_pages_to_render_then_caps_at_max() {
    let huge_count: usize = 500;
    let pages_to_render = huge_count.min(MAX_PAGES_DUE_TO_RAM_USAGE);
    assert_eq!(pages_to_render, MAX_PAGES_DUE_TO_RAM_USAGE);
}

#[tokio::test]
async fn given_page_count_below_limit_when_computing_pages_to_render_then_uses_actual_count() {
    let small_count: usize = 10;
    let pages_to_render = small_count.min(MAX_PAGES_DUE_TO_RAM_USAGE);
    assert_eq!(pages_to_render, 10);
}

#[tokio::test]
async fn given_valid_sharded_index_json_when_parsing_shard_names_then_returns_deduplicated_sorted_names()
 {
    let json = r#"{
        "weight_map": {
            "model.layer.0.weight": "model-00002-of-00002.safetensors",
            "model.layer.1.weight": "model-00001-of-00002.safetensors",
            "model.layer.2.weight": "model-00001-of-00002.safetensors"
        }
    }"#;

    let names = parse_shard_names(json).unwrap();

    assert_eq!(names.len(), 2);
    assert_eq!(names[0], "model-00001-of-00002.safetensors");
    assert_eq!(names[1], "model-00002-of-00002.safetensors");
}

#[tokio::test]
async fn given_index_json_with_empty_weight_map_when_parsing_shard_names_then_returns_error() {
    let json = r#"{ "weight_map": {} }"#;

    let result = parse_shard_names(json);

    assert!(matches!(
        result,
        Err(sandakan::application::ports::FileLoaderError::ExtractionFailed(_))
    ));
}

#[tokio::test]
async fn given_index_json_missing_weight_map_when_parsing_shard_names_then_returns_error() {
    let json = r#"{ "metadata": {} }"#;

    let result = parse_shard_names(json);

    assert!(matches!(
        result,
        Err(sandakan::application::ports::FileLoaderError::ExtractionFailed(_))
    ));
}

#[tokio::test]
async fn given_malformed_json_when_parsing_shard_names_then_returns_error() {
    let result = parse_shard_names("not json at all");

    assert!(matches!(
        result,
        Err(sandakan::application::ports::FileLoaderError::ExtractionFailed(_))
    ));
}
