use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingSettings {
    pub level: String,
    pub enable_json: bool,
    pub enable_udp: bool,
    #[serde(default)]
    pub tempo_endpoint: Option<String>,
}
