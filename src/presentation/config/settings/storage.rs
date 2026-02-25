use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageProviderSetting {
    Local,
    Azure,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageSettings {
    pub provider: StorageProviderSetting,
    pub local_path: String,
    pub max_upload_size_bytes: u64,
    #[serde(default)]
    pub azure_account: Option<String>,
    #[serde(default)]
    pub azure_access_key: Option<String>,
    #[serde(default)]
    pub azure_container: Option<String>,
}
