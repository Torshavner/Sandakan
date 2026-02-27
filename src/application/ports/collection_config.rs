use super::{DistanceMetric, PayloadFieldType, PayloadIndex};

#[derive(Debug, Clone)]
pub struct CollectionConfig {
    pub vector_dimensions: u64,
    pub distance_metric: DistanceMetric,
    pub payload_indexes: Vec<PayloadIndex>,
    pub hybrid: bool,
}

impl CollectionConfig {
    pub fn new(vector_dimensions: u64) -> Self {
        Self {
            vector_dimensions,
            distance_metric: DistanceMetric::Cosine,
            payload_indexes: vec![
                PayloadIndex {
                    field_name: "document_id".to_string(),
                    field_type: PayloadFieldType::Keyword,
                },
                PayloadIndex {
                    field_name: "file_type".to_string(),
                    field_type: PayloadFieldType::Keyword,
                },
                PayloadIndex {
                    field_name: "tenant_id".to_string(),
                    field_type: PayloadFieldType::Keyword,
                },
            ],
            hybrid: false,
        }
    }

    pub fn with_hybrid(mut self) -> Self {
        self.hybrid = true;
        self
    }
}
