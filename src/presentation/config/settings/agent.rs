use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChatMode {
    #[default]
    Rag,
    Agent,
}

// ─── Unified tool configuration ───────────────────────────────────────────────

/// A single entry in the `tools` array. The `"type"` field selects the variant.
///
/// Built-in tools:
///   `{ "type": "rag_search" }`
///   `{ "type": "web_search", "api_key": "...", "max_results": 5 }`
///   `{ "type": "notification", "webhook_url": "...", "format": "slack" }`
///   `{ "type": "fs", "root_path": "./src" }`
///
/// External MCP servers:
///   `{ "type": "mcp_stdio", "name": "github", "command": "npx", "args": [...], "env": {...} }`
///   `{ "type": "mcp_sse",   "name": "my-server", "endpoint": "http://..." }`
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolConfig {
    RagSearch,
    WebSearch(WebSearchConfig),
    Notification(NotificationConfig),
    Fs(FsConfig),
    McpStdio(McpStdioConfig),
    McpSse(McpSseConfig),
}

// ─── Web search ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct WebSearchConfig {
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationFormat {
    Plain,
    Slack,
}

fn default_notification_format() -> NotificationFormat {
    NotificationFormat::Plain
}

fn default_notification_timeout() -> u64 {
    10
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationConfig {
    pub webhook_url: String,
    #[serde(default = "default_notification_format")]
    pub format: NotificationFormat,
    #[serde(default = "default_notification_timeout")]
    pub timeout_secs: u64,
}

// ─── Filesystem tool ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct FsConfig {
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

// ─── MCP transports ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct McpStdioConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpSseConfig {
    pub name: String,
    pub endpoint: String,
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
    crate::application::services::DEFAULT_CRITIC_PROMPT.to_string()
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
    /// Unified tool list — presence in the array means enabled.
    /// Replaces the old per-tool top-level fields.
    #[serde(default)]
    pub tools: Vec<ToolConfig>,
    #[serde(default)]
    pub reflection: ReflectionSettings,
    /// Which backend handles `/v1/chat/completions` by default.
    #[serde(default)]
    pub chat_mode: ChatMode,
    /// When true, tool descriptions are embedded at startup and only the
    /// top-K most relevant tools are passed to the LLM each iteration.
    #[serde(default)]
    pub semantic_tools: bool,
    /// Maximum tools returned by semantic search per ReAct iteration.
    #[serde(default = "default_max_tool_results")]
    pub max_tool_results: usize,
    /// When true, the names and descriptions of all registered tools are
    /// appended to the system prompt at runtime. Helps weaker models that
    /// do not reliably infer available tools from the API schema alone.
    #[serde(default)]
    pub dynamic_tools_description: bool,
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

fn default_max_tool_results() -> usize {
    5
}

// ─── Service config (single source of truth for AgentService construction) ───

/// Bootstrap-time configuration consumed by `AgentService::new()`.
/// Built from `AgentSettings` in the composition root (`main.rs`).
pub struct AgentServiceConfig {
    pub model_config: String,
    pub max_iterations: usize,
    pub tool_timeout_secs: u64,
    pub tool_fail_fast: bool,
    pub system_prompt: String,
    pub reflection: ReflectionSettings,
    pub max_tool_results: usize,
    pub dynamic_tools_description: bool,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_iterations: default_max_iterations(),
            tool_timeout_secs: default_tool_timeout_secs(),
            tool_fail_fast: false,
            system_prompt: default_agent_system_prompt(),
            tools: Vec::new(),
            reflection: ReflectionSettings::default(),
            chat_mode: ChatMode::default(),
            semantic_tools: false,
            max_tool_results: default_max_tool_results(),
            dynamic_tools_description: false,
        }
    }
}
