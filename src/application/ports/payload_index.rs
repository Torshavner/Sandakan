use super::PayloadFieldType;

#[derive(Debug, Clone)]
pub struct PayloadIndex {
    pub field_name: String,
    pub field_type: PayloadFieldType,
}
