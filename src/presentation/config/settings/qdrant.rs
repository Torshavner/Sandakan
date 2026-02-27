use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct QdrantSettings {
    pub url: String,
    pub collection_name: String,
    #[serde(default)]
    pub hybrid_search: bool,
}
