// @AI-BYPASS-LENGTH
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::join_all;

use crate::application::ports::{
    AgentMessage, ConversationRepository, EvalEventRepository, EvalOutboxRepository, LlmClient,
    LlmClientError, LlmTokenStream, LlmToolResponse, McpClientPort, McpError, RagSourceCollector,
    RepositoryError, ToolRegistry,
};
use crate::domain::{AgentState, Conversation, ConversationId, EvalEvent, Message, MessageRole};

// ─── Public surface ──────────────────────────────────────────────────────────

pub struct AgentChatRequest {
    pub conversation_id: Option<ConversationId>,
    pub user_message: String,
}

pub struct AgentChatResponse {
    pub progress_rx: tokio::sync::mpsc::Receiver<AgentProgressEvent>,
    /// Real token-by-token stream of the final LLM answer.
    pub token_stream: LlmTokenStream,
    pub conversation_id: ConversationId,
}

/// Events emitted during the ReAct loop that the presentation layer forwards
/// as SSE progress messages before the final token stream begins.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentProgressEvent {
    Thinking {
        iteration: usize,
    },
    ToolCall {
        name: String,
    },
    ToolResult {
        name: String,
        truncated_content: String,
    },
    Reflection {
        score: f32,
        needs_correction: bool,
        issues: Vec<String>,
    },
    CorrectionApplied,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("max iterations ({0}) exceeded without final answer")]
    MaxIterationsExceeded(usize),
    #[error("llm error: {0}")]
    Llm(#[from] LlmClientError),
    #[error("tool execution error: {0}")]
    Tool(String),
    #[error("repository error: {0}")]
    Repository(String),
}

impl From<RepositoryError> for AgentError {
    fn from(e: RepositoryError) -> Self {
        AgentError::Repository(e.to_string())
    }
}

impl From<McpError> for AgentError {
    fn from(e: McpError) -> Self {
        AgentError::Tool(e.to_string())
    }
}

// ─── Port (thin trait for AppState to avoid 5th generic) ─────────────────────

#[async_trait]
pub trait AgentServicePort: Send + Sync {
    async fn chat(&self, request: AgentChatRequest) -> Result<AgentChatResponse, AgentError>;
}

// ─── Service config ───────────────────────────────────────────────────────────

pub struct ReflectionSettings {
    pub enabled: bool,
    /// Minimum score (0.0–1.0) for an answer to be returned without correction.
    pub score_threshold: f32,
    /// Maximum number of correction passes per turn (prevents run-away LLM cost).
    pub correction_budget: usize,
    /// System prompt sent to the critic LLM call.
    pub critic_system_prompt: String,
}

impl Default for ReflectionSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            score_threshold: 0.7,
            correction_budget: 1,
            critic_system_prompt: DEFAULT_CRITIC_PROMPT.to_string(),
        }
    }
}

pub struct AgentServiceConfig {
    pub model_config: String,
    pub max_iterations: usize,
    /// Per-tool call timeout. A timed-out tool is surfaced as a `[tool_timeout]`
    /// ToolResult rather than aborting the whole agent turn.
    pub tool_timeout_secs: u64,
    /// When `true`, any tool error (except `ToolNotFound`) hard-fails the turn.
    /// When `false` (default), errors are surfaced as `[tool_error]` ToolResults.
    pub tool_fail_fast: bool,
    /// System prompt prepended as the first message on every agent turn.
    /// Instructs the LLM about its role, available tools, and reasoning approach.
    pub system_prompt: String,
    pub reflection: ReflectionSettings,
}

// ─── Concrete service ─────────────────────────────────────────────────────────

pub struct AgentService {
    llm_client: Arc<dyn LlmClient>,
    mcp_client: Arc<dyn McpClientPort>,
    tool_registry: Arc<dyn ToolRegistry>,
    conversation_repository: Arc<dyn ConversationRepository>,
    eval_event_repository: Option<Arc<dyn EvalEventRepository>>,
    eval_outbox_repository: Option<Arc<dyn EvalOutboxRepository>>,
    rag_source_collector: Option<Arc<dyn RagSourceCollector>>,
    config: AgentServiceConfig,
}

impl AgentService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        llm_client: Arc<dyn LlmClient>,
        mcp_client: Arc<dyn McpClientPort>,
        tool_registry: Arc<dyn ToolRegistry>,
        conversation_repository: Arc<dyn ConversationRepository>,
        eval_event_repository: Option<Arc<dyn EvalEventRepository>>,
        eval_outbox_repository: Option<Arc<dyn EvalOutboxRepository>>,
        rag_source_collector: Option<Arc<dyn RagSourceCollector>>,
        config: AgentServiceConfig,
    ) -> Self {
        Self {
            llm_client,
            mcp_client,
            tool_registry,
            conversation_repository,
            eval_event_repository,
            eval_outbox_repository,
            rag_source_collector,
            config,
        }
    }

    /// Runs the ReAct loop and returns `(buffered_answer, final_messages)`.
    ///
    /// `final_messages` includes the full conversation history up to and
    /// including the last user turn; the caller uses it to produce a real
    /// token-by-token stream via `complete_stream_with_messages`.
    #[tracing::instrument(skip(self), fields(conversation_id))]
    async fn run_react_loop(
        &self,
        user_message: String,
        conversation_id: ConversationId,
        progress_tx: &tokio::sync::mpsc::Sender<AgentProgressEvent>,
    ) -> Result<(String, Vec<AgentMessage>), AgentError> {
        let history = self
            .conversation_repository
            .get_messages(conversation_id, 50)
            .await
            .map_err(AgentError::from)?;

        let mut messages: Vec<AgentMessage> =
            std::iter::once(AgentMessage::System(self.config.system_prompt.clone()))
                .chain(history.into_iter().map(AgentMessage::from))
                .collect();
        messages.push(AgentMessage::User(user_message));

        let tools = self.tool_registry.list_tools();
        let mut _state = AgentState::Thinking;
        let timeout_dur = Duration::from_secs(self.config.tool_timeout_secs);

        for iteration in 0..self.config.max_iterations {
            // Discard send errors — the handler may have disconnected.
            let _ = progress_tx.try_send(AgentProgressEvent::Thinking { iteration });

            match self
                .llm_client
                .complete_with_tools(&messages, &tools)
                .await?
            {
                LlmToolResponse::ToolCalls(calls) => {
                    _state = AgentState::AwaitingToolExecution;

                    // Append the assistant's tool-call intent to message history.
                    messages.push(AgentMessage::Assistant {
                        content: None,
                        tool_calls: calls.clone(),
                    });

                    // Emit ToolCall progress events before dispatching.
                    for call in &calls {
                        let _ = progress_tx.try_send(AgentProgressEvent::ToolCall {
                            name: call.name.to_string(),
                        });
                    }

                    // Execute all tool calls concurrently with per-call timeouts.
                    let outcomes: Vec<_> = join_all(calls.iter().map(|call| {
                        tokio::time::timeout(timeout_dur, self.mcp_client.call_tool(call))
                    }))
                    .await;

                    for (call, outcome) in calls.iter().zip(outcomes) {
                        let tool_result = match outcome {
                            Ok(Ok(r)) => r,
                            Ok(Err(McpError::ToolNotFound(n))) => {
                                // A missing tool is always a hard error regardless of fail_fast.
                                return Err(AgentError::Tool(format!("tool not found: {n}")));
                            }
                            Ok(Err(e)) if self.config.tool_fail_fast => {
                                return Err(AgentError::from(e));
                            }
                            Ok(Err(e)) => crate::domain::ToolResult {
                                tool_call_id: call.id.clone(),
                                tool_name: call.name.clone(),
                                content: format!("[tool_error] {}: {e}", call.name),
                            },
                            Err(_elapsed) => crate::domain::ToolResult {
                                tool_call_id: call.id.clone(),
                                tool_name: call.name.clone(),
                                content: format!(
                                    "[tool_timeout] {} did not respond within {}s",
                                    call.name, self.config.tool_timeout_secs
                                ),
                            },
                        };

                        let truncated = truncate_for_event(&tool_result.content, 120);
                        let _ = progress_tx.try_send(AgentProgressEvent::ToolResult {
                            name: tool_result.tool_name.to_string(),
                            truncated_content: truncated,
                        });

                        messages.push(AgentMessage::ToolResult(tool_result));
                    }

                    _state = AgentState::Thinking;
                }

                LlmToolResponse::Content(answer) => {
                    _state = AgentState::YieldingResponse;
                    return Ok((answer, messages));
                }
            }
        }

        Err(AgentError::MaxIterationsExceeded(
            self.config.max_iterations,
        ))
    }

    async fn persist_turn(
        &self,
        conversation_id: ConversationId,
        user_message: &str,
        answer: &str,
    ) {
        let user_msg = Message::new(conversation_id, MessageRole::User, user_message.to_string());
        let assistant_msg =
            Message::new(conversation_id, MessageRole::Assistant, answer.to_string());

        if let Err(e) = self.conversation_repository.append_message(&user_msg).await {
            tracing::warn!(error = %e, "Failed to persist agent user message");
        }
        if let Err(e) = self
            .conversation_repository
            .append_message(&assistant_msg)
            .await
        {
            tracing::warn!(error = %e, "Failed to persist agent assistant message");
        }
    }

    fn fire_and_forget_eval(&self, question: &str, answer: &str) {
        if let (Some(event_repo), Some(outbox_repo)) =
            (&self.eval_event_repository, &self.eval_outbox_repository)
        {
            let sources = self
                .rag_source_collector
                .as_ref()
                .map(|c| c.drain())
                .unwrap_or_default();

            let eval_event =
                EvalEvent::new_agentic(question, answer, sources, &self.config.model_config);
            let event_repo = Arc::clone(event_repo);
            let outbox_repo = Arc::clone(outbox_repo);
            tokio::spawn(async move {
                match event_repo.record(&eval_event).await {
                    Ok(_) => {
                        if let Err(e) = outbox_repo.enqueue(eval_event.id).await {
                            tracing::warn!(error = %e, "Failed to enqueue agent eval outbox");
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "Failed to record agent eval event"),
                }
            });
        }
    }

    /// Performs a critic pass on the candidate answer and, if the score falls
    /// below the configured threshold and the budget allows, appends the critic's
    /// feedback and runs one correction iteration.
    ///
    /// Gracefully degrades: any failure to parse the critic response is treated
    /// as score = 1.0 so the original answer is returned unchanged.
    async fn reflect_and_correct(
        &self,
        candidate_answer: String,
        mut messages: Vec<AgentMessage>,
        progress_tx: &tokio::sync::mpsc::Sender<AgentProgressEvent>,
    ) -> Result<(String, Vec<AgentMessage>), AgentError> {
        let cfg = &self.config.reflection;

        let critic_prompt = format!(
            "{}\n\nCandidate answer:\n{}",
            cfg.critic_system_prompt, candidate_answer
        );

        let raw = match self.llm_client.complete(&critic_prompt, "").await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "Critic LLM call failed; skipping reflection");
                return Ok((candidate_answer, messages));
            }
        };

        let (score, issues) = parse_critic_response(&raw);

        let needs_correction = score < cfg.score_threshold && cfg.correction_budget > 0;

        let _ = progress_tx.try_send(AgentProgressEvent::Reflection {
            score,
            needs_correction,
            issues: issues.clone(),
        });

        if !needs_correction {
            return Ok((candidate_answer, messages));
        }

        let feedback = format!(
            "Your previous answer scored {score:.2}/1.0 for completeness and grounding. Issues noted:\n{}\n\nPlease provide a corrected, more complete answer.",
            issues.join(", ")
        );
        messages.push(AgentMessage::User(feedback));

        let tools = self.tool_registry.list_tools();
        let refined = match self
            .llm_client
            .complete_with_tools(&messages, &tools)
            .await?
        {
            LlmToolResponse::Content(r) => r,
            // If the LLM calls a tool instead of correcting, return the original answer.
            LlmToolResponse::ToolCalls(_) => candidate_answer,
        };

        let _ = progress_tx.try_send(AgentProgressEvent::CorrectionApplied);

        Ok((refined, messages))
    }
}

#[async_trait]
impl AgentServicePort for AgentService {
    async fn chat(&self, request: AgentChatRequest) -> Result<AgentChatResponse, AgentError> {
        let conversation_id = match request.conversation_id {
            Some(id) => id,
            None => {
                let conv = Conversation::new(None);
                let id = conv.id;
                if let Err(e) = self
                    .conversation_repository
                    .create_conversation(&conv)
                    .await
                {
                    tracing::warn!(error = %e, "Failed to create agent conversation");
                }
                id
            }
        };

        // Bounded channel — progress events are cheap and numerous, 64 slots is ample.
        let (progress_tx, progress_rx) = tokio::sync::mpsc::channel(64);

        let (candidate_answer, candidate_messages) = self
            .run_react_loop(request.user_message.clone(), conversation_id, &progress_tx)
            .await?;

        let (answer, final_messages) = if self.config.reflection.enabled {
            self.reflect_and_correct(candidate_answer, candidate_messages, &progress_tx)
                .await?
        } else {
            (candidate_answer, candidate_messages)
        };

        // Drop sender so the handler's drain loop sees the channel as closed.
        drop(progress_tx);

        self.persist_turn(conversation_id, &request.user_message, &answer)
            .await;

        self.fire_and_forget_eval(&request.user_message, &answer);

        // Stream the final answer token-by-token using the full conversation history.
        // The buffered `answer` above is used only for persistence; the streaming
        // call independently produces the SSE token output seen by the client.
        let token_stream: LlmTokenStream = self
            .llm_client
            .complete_stream_with_messages(&final_messages)
            .await?;

        Ok(AgentChatResponse {
            progress_rx,
            token_stream,
            conversation_id,
        })
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

pub const DEFAULT_AGENT_SYSTEM_PROMPT: &str = "\
You are a helpful AI assistant with access to tools. \
Use tools when they help you answer the user's question more accurately or completely. \
Reason step-by-step: think about what information you need, call the appropriate tools, \
observe the results, and synthesise a final answer. \
When you have enough information to answer, respond directly without calling additional tools. \
Always cite relevant sources from retrieved content when available.";

const DEFAULT_CRITIC_PROMPT: &str = "\
You are a critical evaluator. Review the candidate answer below and score it from 0.0 to 1.0 based on:\
\n- Completeness: does it address the full question?\
\n- Grounding: is it consistent with what was retrieved (no hallucination)?\
\n- Clarity: is it clear and actionable?\
\n\nRespond ONLY in this format:\
\nSCORE: 0.X\
\nISSUES: <comma-separated list, or \"none\">";

/// Truncates `s` to at most `max_bytes` bytes without splitting a UTF-8 codepoint.
///
/// `s.len()` is a byte count, so a naive `&s[..max_bytes]` panics when the cut
/// lands inside a multi-byte sequence (CJK, emoji, accented text). Walking
/// `char_indices` finds the last safe codepoint boundary at or before the limit.
fn truncate_for_event(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // Find the byte offset of the last char that fits entirely within max_bytes.
    let boundary = s
        .char_indices()
        .take_while(|(byte_pos, ch)| byte_pos + ch.len_utf8() <= max_bytes)
        .last()
        .map(|(byte_pos, ch)| byte_pos + ch.len_utf8())
        .unwrap_or(0);
    format!("{}…", &s[..boundary])
}

/// Parses `SCORE: 0.X` and `ISSUES: ...` lines from a critic response.
///
/// Returns `(1.0, [])` on any parse failure so the caller treats the answer as
/// passing and skips the correction pass (graceful degradation).
fn parse_critic_response(raw: &str) -> (f32, Vec<String>) {
    let mut score: f32 = 1.0;
    let mut issues: Vec<String> = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("SCORE:") {
            if let Ok(v) = rest.trim().parse::<f32>() {
                score = v.clamp(0.0, 1.0);
            }
        } else if let Some(rest) = trimmed.strip_prefix("ISSUES:") {
            let rest = rest.trim();
            if !rest.eq_ignore_ascii_case("none") {
                issues = rest
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }

    (score, issues)
}
