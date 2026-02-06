/// Configuration for tracing initialization.
pub struct TracingConfig {
    pub environment: String,
    pub json_format: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            environment: std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
            json_format: std::env::var("LOG_FORMAT")
                .map(|v| v.to_lowercase() == "json")
                .unwrap_or(false),
        }
    }
}
