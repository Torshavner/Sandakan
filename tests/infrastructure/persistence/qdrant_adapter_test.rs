use sandakan::application::ports::{
    CollectionConfig, DistanceMetric, PayloadFieldType, PayloadIndex,
};

#[test]
fn given_openai_default_config_when_created_then_has_1536_dimensions() {
    let config = CollectionConfig::openai_default();

    assert_eq!(config.vector_dimensions, 1536);
}

#[test]
fn given_openai_default_config_when_created_then_uses_cosine_distance() {
    let config = CollectionConfig::openai_default();

    assert_eq!(config.distance_metric, DistanceMetric::Cosine);
}

#[test]
fn given_openai_default_config_when_created_then_has_required_payload_indexes() {
    let config = CollectionConfig::openai_default();

    let field_names: Vec<&str> = config
        .payload_indexes
        .iter()
        .map(|idx| idx.field_name.as_str())
        .collect();

    assert!(field_names.contains(&"document_id"));
    assert!(field_names.contains(&"file_type"));
    assert!(field_names.contains(&"tenant_id"));
}

#[test]
fn given_payload_index_when_created_then_stores_field_name_and_type() {
    let index = PayloadIndex {
        field_name: "test_field".to_string(),
        field_type: PayloadFieldType::Keyword,
    };

    assert_eq!(index.field_name, "test_field");
    assert_eq!(index.field_type, PayloadFieldType::Keyword);
}

#[test]
fn given_collection_config_when_customized_then_stores_custom_values() {
    let config = CollectionConfig {
        vector_dimensions: 768,
        distance_metric: DistanceMetric::Euclidean,
        payload_indexes: vec![PayloadIndex {
            field_name: "custom_field".to_string(),
            field_type: PayloadFieldType::Integer,
        }],
    };

    assert_eq!(config.vector_dimensions, 768);
    assert_eq!(config.distance_metric, DistanceMetric::Euclidean);
    assert_eq!(config.payload_indexes.len(), 1);
}

#[test]
fn given_distance_metrics_when_compared_then_are_distinct() {
    assert_ne!(DistanceMetric::Cosine, DistanceMetric::Euclidean);
    assert_ne!(DistanceMetric::Euclidean, DistanceMetric::DotProduct);
    assert_ne!(DistanceMetric::Cosine, DistanceMetric::DotProduct);
}

#[test]
fn given_payload_field_types_when_compared_then_are_distinct() {
    assert_ne!(PayloadFieldType::Keyword, PayloadFieldType::Integer);
    assert_ne!(PayloadFieldType::Integer, PayloadFieldType::Float);
    assert_ne!(PayloadFieldType::Float, PayloadFieldType::Text);
}
