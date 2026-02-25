use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub server: ServerSettings,
    pub qdrant: QdrantSettings,
    pub database: DatabaseSettings,
    pub embeddings: EmbeddingsSettings,
    pub chunking: ChunkingSettings,
    pub llm: LlmSettings,
    pub logging: LoggingSettings,
    pub extraction: ExtractionSettings,
    pub storage: StorageSettings,
    pub rag: RagSettings,
    #[serde(default)]
    pub eval: EvalSettings,
    #[serde(default)]
    pub agent: AgentSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QdrantSettings {
    pub url: String,
    pub collection_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddingsSettings {
    pub provider: EmbeddingProvider,
    pub model: String,
    pub dimension: usize,
    pub chunk_overlap: usize,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingProvider {
    Local,
    #[serde(rename = "openai")]
    OpenAi,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChunkingStrategy {
    Semantic,
    Fixed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChunkingSettings {
    pub max_chunk_size: usize,
    pub overlap_tokens: usize,
    pub strategy: ChunkingStrategy,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmSettings {
    pub provider: String,
    pub api_key: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub azure_endpoint: Option<String>,
    pub chat_model: String,
    pub max_tokens: usize,
    pub temperature: f32,
    #[serde(default = "default_sse_keep_alive")]
    pub sse_keep_alive_seconds: u64,
}

fn default_sse_keep_alive() -> u64 {
    15
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingSettings {
    pub level: String,
    pub enable_json: bool,
    pub enable_udp: bool,
    #[serde(default)]
    pub tempo_endpoint: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractionSettings {
    pub pdf: PdfExtractionSettings,
    pub audio: AudioExtractionSettings,
    pub video: VideoExtractionSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PdfExtractionSettings {
    pub enabled: bool,
    pub max_file_size_mb: usize,
    #[serde(default = "default_extractor_provider")]
    pub provider: ExtractorProvider,
    #[serde(default)]
    pub vlm_model: Option<String>,
    #[serde(default)]
    pub vlm_revision: Option<String>,
    #[serde(default)]
    pub vlm_base_url: Option<String>,
    #[serde(default)]
    pub vlm_api_key: Option<String>,
    #[serde(default)]
    pub azure_endpoint: Option<String>,
    #[serde(default)]
    pub azure_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AudioExtractionSettings {
    pub enabled: bool,
    pub max_file_size_mb: usize,
    pub whisper_model: String,
    #[serde(default = "default_transcription_provider")]
    pub provider: TranscriptionProviderSetting,
    #[serde(default)]
    pub azure_endpoint: Option<String>,
    #[serde(default)]
    pub azure_deployment: Option<String>,
    #[serde(default)]
    pub azure_key: Option<String>,
    #[serde(default)]
    pub azure_api_version: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionProviderSetting {
    Local,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "azure")]
    Azure,
}

fn default_transcription_provider() -> TranscriptionProviderSetting {
    TranscriptionProviderSetting::Local
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractorProvider {
    LocalVlm,
    LmStudio,
    Azure,
}

fn default_extractor_provider() -> ExtractorProvider {
    ExtractorProvider::LocalVlm
}

#[derive(Debug, Clone, Deserialize)]
pub struct VideoExtractionSettings {
    pub enabled: bool,
    pub max_file_size_mb: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RagSettings {
    pub similarity_threshold: f32,
    pub max_context_tokens: usize,
    pub top_k: usize,
    pub system_prompt: String,
    pub fallback_message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSettings {
    pub url: String,
    pub max_connections: u32,
    pub run_migrations: bool,
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

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageProviderSetting {
    Local,
    Azure,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EvalSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_faithfulness_threshold")]
    pub faithfulness_threshold: f32,
    #[serde(default = "default_correctness_threshold")]
    pub correctness_threshold: f32,
    #[serde(default = "default_poll_interval")]
    pub worker_poll_interval_secs: u64,
    #[serde(default = "default_batch_size")]
    pub worker_batch_size: usize,
}

fn default_faithfulness_threshold() -> f32 {
    0.7
}

fn default_correctness_threshold() -> f32 {
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
            correctness_threshold: default_correctness_threshold(),
            worker_poll_interval_secs: default_poll_interval(),
            worker_batch_size: default_batch_size(),
        }
    }
}

// ─── Agent settings ───────────────────────────────────────────────────────────

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
    "You are a critical evaluator. Review the candidate answer below and score it from 0.0 to 1.0 based on:\n- Completeness: does it address the full question?\n- Grounding: is it consistent with what was retrieved (no hallucination)?\n- Clarity: is it clear and actionable?\n\nRespond ONLY in this format:\nSCORE: 0.X\nISSUES: <comma-separated list, or \"none\">".to_string()
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

#[derive(Debug, Clone, Deserialize)]
pub struct WebSearchSettings {
    pub api_key: String,
    #[serde(default = "default_search_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_search_max_results")]
    pub max_results: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationSettings {
    pub webhook_url: String,
    #[serde(default = "default_notification_format")]
    pub format: NotificationFormatSetting,
    #[serde(default = "default_notification_timeout")]
    pub timeout_secs: u64,
}

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

fn default_max_iterations() -> usize {
    10
}

fn default_tool_timeout_secs() -> u64 {
    30
}

fn default_search_endpoint() -> String {
    "https://api.search.brave.com/res/v1/web/search".to_string()
}

fn default_search_max_results() -> usize {
    5
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_iterations: default_max_iterations(),
            tool_timeout_secs: default_tool_timeout_secs(),
            tool_fail_fast: false,
            web_search: None,
            rag_search_enabled: false,
            notification: None,
            mcp_servers: Vec::new(),
            fs_tools: None,
            reflection: ReflectionSettings::default(),
        }
    }
}
