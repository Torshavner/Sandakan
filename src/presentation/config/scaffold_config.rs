/// Configuration for scaffold mode operation.
#[derive(Debug, Clone)]
pub struct ScaffoldConfig {
    pub enabled: bool,
    pub mock_response_delay_ms: u64,
}

impl Default for ScaffoldConfig {
    fn default() -> Self {
        Self {
            enabled: std::env::var("SCAFFOLD_MODE")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(false),
            mock_response_delay_ms: std::env::var("MOCK_RESPONSE_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        }
    }
}
