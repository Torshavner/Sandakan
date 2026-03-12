use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct EvalSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_faithfulness_threshold")]
    pub faithfulness_threshold: f32,
    #[serde(default = "default_poll_interval")]
    pub worker_poll_interval_secs: u64,
    #[serde(default = "default_batch_size")]
    pub worker_batch_size: usize,
}

fn default_faithfulness_threshold() -> f32 {
    0.7
}

fn default_poll_interval() -> u64 {
    30
}

fn default_batch_size() -> usize {
    10
}

impl Default for EvalSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            faithfulness_threshold: default_faithfulness_threshold(),
            worker_poll_interval_secs: default_poll_interval(),
            worker_batch_size: default_batch_size(),
        }
    }
}
