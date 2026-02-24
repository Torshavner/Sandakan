// @AI-BYPASS-LENGTH
use std::sync::Arc;

use async_trait::async_trait;
use futures::stream;

use crate::application::ports::{
    AgentMessage, ConversationRepository, EvalEventRepository, EvalOutboxRepository, LlmClient,
    LlmClientError, LlmTokenStream, LlmToolResponse, McpClientPort, McpError, RepositoryError,
    ToolRegistry,
};
use crate::domain::{
    AgentState, Conversation, ConversationId, EvalEvent, EvalSource, Message, MessageRole,
};

// ─── Public surface ──────────────────────────────────────────────────────────

pub struct AgentChatRequest {
    pub conversation_id: Option<ConversationId>,
    pub user_message: String,
}

pub struct AgentChatResponse {
    pub progress_rx: tokio::sync::mpsc::Receiver<AgentProgressEvent>,
    /// Single-chunk stream containing the final LLM answer.
    ///
    /// TODO: US-022 — replace with a real token-by-token streaming call.
    pub token_stream: LlmTokenStream,
    pub conversation_id: ConversationId,
}

/// Events emitted during the ReAct loop that the presentation layer forwards
/// as SSE progress messages before the final token stream begins.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentProgressEvent {
    Thinking { iteration: usize },
    ToolCall { name: String },
    ToolResult { name: String, truncated_content: String },
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

// ─── Concrete service ─────────────────────────────────────────────────────────

pub struct AgentService {
    llm_client: Arc<dyn LlmClient>,
    mcp_client: Arc<dyn McpClientPort>,
    tool_registry: Arc<dyn ToolRegistry>,
    conversation_repository: Arc<dyn ConversationRepository>,
    eval_event_repository: Option<Arc<dyn EvalEventRepository>>,
    eval_outbox_repository: Option<Arc<dyn EvalOutboxRepository>>,
    model_config: String,
    max_iterations: usize,
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
        model_config: String,
        max_iterations: usize,
    ) -> Self {
        Self {
            llm_client,
            mcp_client,
            tool_registry,
            conversation_repository,
            eval_event_repository,
            eval_outbox_repository,
            model_config,
            max_iterations,
        }
    }

    #[tracing::instrument(skip(self), fields(conversation_id))]
    async fn run_react_loop(
        &self,
        user_message: String,
        conversation_id: ConversationId,
        progress_tx: &tokio::sync::mpsc::Sender<AgentProgressEvent>,
    ) -> Result<String, AgentError> {
        let history = self
            .conversation_repository
            .get_messages(conversation_id, 50)
            .await
            .map_err(AgentError::from)?;

        let mut messages: Vec<AgentMessage> = history.into_iter().map(AgentMessage::from).collect();
        messages.push(AgentMessage::User(user_message));

        let tools = self.tool_registry.list_tools();
        let mut _state = AgentState::Thinking;

        for iteration in 0..self.max_iterations {
            // Discard send errors — the handler may have disconnected.
            let _ = progress_tx
                .try_send(AgentProgressEvent::Thinking { iteration });

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

                    // Execute each tool call sequentially.
                    // TODO: US-022 — parallelise with futures::future::join_all().
                    for call in &calls {
                        let _ = progress_tx.try_send(AgentProgressEvent::ToolCall {
                            name: call.name.to_string(),
                        });

                        let result = self.mcp_client.call_tool(call).await?;

                        let truncated = truncate_for_event(&result.content, 120);
                        let _ = progress_tx.try_send(AgentProgressEvent::ToolResult {
                            name: result.tool_name.to_string(),
                            truncated_content: truncated,
                        });

                        messages.push(AgentMessage::ToolResult(result));
                    }

                    _state = AgentState::Thinking;
                }

                LlmToolResponse::Content(answer) => {
                    _state = AgentState::YieldingResponse;
                    return Ok(answer);
                }
            }
        }

        Err(AgentError::MaxIterationsExceeded(self.max_iterations))
    }

    async fn persist_turn(
        &self,
        conversation_id: ConversationId,
        user_message: &str,
        answer: &str,
    ) {
        let user_msg = Message::new(
            conversation_id,
            MessageRole::User,
            user_message.to_string(),
        );
        let assistant_msg = Message::new(
            conversation_id,
            MessageRole::Assistant,
            answer.to_string(),
        );

        if let Err(e) = self
            .conversation_repository
            .append_message(&user_msg)
            .await
        {
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
            let eval_event = EvalEvent::new(question, answer, Vec::<EvalSource>::new(), &self.model_config);
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

        let answer = self
            .run_react_loop(request.user_message.clone(), conversation_id, &progress_tx)
            .await?;

        // Drop sender so the handler's drain loop sees the channel as closed.
        drop(progress_tx);

        self.persist_turn(conversation_id, &request.user_message, &answer)
            .await;

        self.fire_and_forget_eval(&request.user_message, &answer);

        // Wrap the answer in a single-chunk stream.
        // TODO: US-022 — replace with real token-by-token streaming via a second LLM call.
        let token_stream: LlmTokenStream =
            Box::pin(stream::once(async move { Ok::<String, LlmClientError>(answer) }));

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
