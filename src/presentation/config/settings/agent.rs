use serde::Deserialize;

// ─── MCP transports ───────────────────────────────────────────────────────────

/// Discriminated union over the two MCP wire transports.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpServerConfig {
    Stdio(StdioMcpServerConfig),
    Sse(SseMcpServerConfig),
}

impl McpServerConfig {
    pub fn name(&self) -> &str {
        match self {
            Self::Stdio(c) => &c.name,
            Self::Sse(c) => &c.name,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StdioMcpServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SseMcpServerConfig {
    pub name: String,
    pub endpoint: String,
}

// ─── Web search ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct WebSearchSettings {
    pub api_key: String,
    #[serde(default = "default_search_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_search_max_results")]
    pub max_results: usize,
}

fn default_search_endpoint() -> String {
    "https://api.search.brave.com/res/v1/web/search".to_string()
}

fn default_search_max_results() -> usize {
    5
}

// ─── Notification ─────────────────────────────────────────────────────────────

/// Serialised form of the notification body format.
///
/// Kept separate from the domain enum so serde rename_all applies at
/// the config boundary only and the adapter stays format-agnostic.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationFormatSetting {
    Plain,
    Slack,
}

fn default_notification_format() -> NotificationFormatSetting {
    NotificationFormatSetting::Plain
}

fn default_notification_timeout() -> u64 {
    10
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationSettings {
    pub webhook_url: String,
    #[serde(default = "default_notification_format")]
    pub format: NotificationFormatSetting,
    #[serde(default = "default_notification_timeout")]
    pub timeout_secs: u64,
}

// ─── Filesystem tool ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct FsToolSettings {
    pub root_path: String,
    #[serde(default = "default_max_read_bytes")]
    pub max_read_bytes: usize,
    #[serde(default = "default_max_dir_entries")]
    pub max_dir_entries: usize,
}

fn default_max_read_bytes() -> usize {
    32_768
}

fn default_max_dir_entries() -> usize {
    200
}

// ─── Reflection ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ReflectionSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_reflection_score_threshold")]
    pub score_threshold: f32,
    #[serde(default = "default_reflection_correction_budget")]
    pub correction_budget: usize,
    #[serde(default = "default_critic_system_prompt")]
    pub critic_system_prompt: String,
}

fn default_reflection_score_threshold() -> f32 {
    0.7
}

fn default_reflection_correction_budget() -> usize {
    1
}

fn default_critic_system_prompt() -> String {
    "You are a critical evaluator. Review the candidate answer below and score it from 0.0 to 1.0 based on:\n\
     - Completeness: does it address the full question?\n\
     - Grounding: is it consistent with what was retrieved (no hallucination)?\n\
     - Clarity: is it clear and actionable?\n\n\
     Respond ONLY in this format:\n\
     SCORE: 0.X\n\
     ISSUES: <comma-separated list, or \"none\">"
        .to_string()
}

impl Default for ReflectionSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            score_threshold: default_reflection_score_threshold(),
            correction_budget: default_reflection_correction_budget(),
            critic_system_prompt: default_critic_system_prompt(),
        }
    }
}

// ─── Agent ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct AgentSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
    #[serde(default = "default_tool_timeout_secs")]
    pub tool_timeout_secs: u64,
    #[serde(default)]
    pub tool_fail_fast: bool,
    #[serde(default = "default_agent_system_prompt")]
    pub system_prompt: String,
    #[serde(default)]
    pub web_search: Option<WebSearchSettings>,
    #[serde(default)]
    pub rag_search_enabled: bool,
    #[serde(default)]
    pub notification: Option<NotificationSettings>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default)]
    pub fs_tools: Option<FsToolSettings>,
    #[serde(default)]
    pub reflection: ReflectionSettings,
}

fn default_agent_system_prompt() -> String {
    crate::application::services::DEFAULT_AGENT_SYSTEM_PROMPT.to_string()
}

fn default_max_iterations() -> usize {
    10
}

fn default_tool_timeout_secs() -> u64 {
    30
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_iterations: default_max_iterations(),
            tool_timeout_secs: default_tool_timeout_secs(),
            tool_fail_fast: false,
            system_prompt: default_agent_system_prompt(),
            web_search: None,
            rag_search_enabled: false,
            notification: None,
            mcp_servers: Vec::new(),
            fs_tools: None,
            reflection: ReflectionSettings::default(),
        }
    }
}
