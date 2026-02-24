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

pub struct AgentServiceConfig {
    pub model_config: String,
    pub max_iterations: usize,
    /// Per-tool call timeout. A timed-out tool is surfaced as a `[tool_timeout]`
    /// ToolResult rather than aborting the whole agent turn.
    pub tool_timeout_secs: u64,
    /// When `true`, any tool error (except `ToolNotFound`) hard-fails the turn.
    /// When `false` (default), errors are surfaced as `[tool_error]` ToolResults.
    pub tool_fail_fast: bool,
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

        let mut messages: Vec<AgentMessage> = history.into_iter().map(AgentMessage::from).collect();
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

            let eval_event = EvalEvent::new(question, answer, sources, &self.config.model_config);
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

        let (answer, final_messages) = self
            .run_react_loop(request.user_message.clone(), conversation_id, &progress_tx)
            .await?;

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

fn truncate_for_event(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", &s[..max_chars])
    }
}
