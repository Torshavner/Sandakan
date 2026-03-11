// @AI-BYPASS-LENGTH
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::join_all;
use tracing::Instrument;

use crate::application::errors::AgentError;
use crate::application::ports::{
    AgentMessage, ConversationRepository, EvalEventRepository, EvalOutboxRepository, LlmClient,
    LlmClientError, LlmTokenStream, LlmToolResponse, McpClientPort, McpError, RagSourceCollector,
    ToolRegistry,
};
use crate::domain::{Conversation, ConversationId, EvalEvent, Message, MessageRole, ToolName};
use crate::presentation::config::AgentServiceConfig;

use super::react_helpers::{
    all_tool_results_failed, build_critic_prompt, parse_critic_response, truncate_for_event,
};
use super::schema::{AgentChatRequest, AgentChatResponse, AgentProgressEvent, AgentServicePort};

// ─── Constants ───────────────────────────────────────────────────────────────

pub const DEFAULT_AGENT_SYSTEM_PROMPT: &str = "\
You are a helpful AI assistant built for the Ciklum AI Academy. \
Always begin your response by identifying yourself as the Ciklum AI Academy assistant. \
Always use the available tools — never answer from memory alone. \
Reason step-by-step: think about what information you need, call the appropriate tools, \
observe the results, and synthesise a final answer. \
When you have enough information to answer, respond directly without calling additional tools. \
Always cite relevant sources from retrieved content when available.";

pub const DEFAULT_CRITIC_PROMPT: &str = "\
You are a critical evaluator. Review the candidate answer below and score it from 0.0 to 1.0 based on:\
\n- Completeness: does it address the full question?\
\n- Grounding: is it consistent with what was retrieved (no hallucination)?\
\n- Clarity: is it clear and actionable?\
\n\nRespond ONLY in this format:\
\nSCORE: 0.X\
\nISSUES: <comma-separated list, or \"none\">";

// ─── Concrete service ────────────────────────────────────────────────────────

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

        let system_prompt = if self.config.dynamic_tools_description {
            let tool_list = self
                .tool_registry
                .list_tools()
                .into_iter()
                .map(|t| format!("- {}: {}", t.name, t.description))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "{}\n\nAvailable tools:\n{}",
                self.config.system_prompt, tool_list
            )
        } else {
            self.config.system_prompt.clone()
        };

        let mut messages: Vec<AgentMessage> =
            std::iter::once(AgentMessage::System(system_prompt))
                .chain(history.into_iter().map(AgentMessage::from))
                .collect();
        messages.push(AgentMessage::User(user_message));

        let timeout_dur = Duration::from_secs(self.config.tool_timeout_secs);

        for iteration in 0..self.config.max_iterations {
            // Discard send errors — the handler may have disconnected.
            let _ = progress_tx.try_send(AgentProgressEvent::Thinking { iteration });

            // Retrieve tools relevant to the current conversation state.
            // On the first iteration use the user message; on subsequent
            // iterations use the latest message (which may be a tool result
            // or nudge) to refine tool selection.
            let current_intent = messages
                .iter()
                .rev()
                .find_map(|m| match m {
                    AgentMessage::User(text) => Some(text.as_str()),
                    _ => None,
                })
                .unwrap_or("");
            let tools = self
                .tool_registry
                .search_tools(current_intent, self.config.max_tool_results)
                .await;

            match self
                .llm_client
                .complete_with_tools(&messages, &tools)
                .await?
            {
                LlmToolResponse::ToolCalls(calls) => {
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

                    // Quality gate: nudge the LLM when every tool call failed.
                    let recent_results: Vec<&crate::domain::ToolResult> = messages
                        .iter()
                        .rev()
                        .take(calls.len())
                        .filter_map(|m| match m {
                            AgentMessage::ToolResult(r) => Some(r),
                            _ => None,
                        })
                        .collect();
                    if all_tool_results_failed(&recent_results) {
                        messages.push(AgentMessage::User(
                            "All tool calls failed or timed out. \
                             Try rephrasing your query or using a different tool."
                                .to_string(),
                        ));
                    }
                }

                LlmToolResponse::Content(answer) => {
                    return Ok((answer, messages));
                }
            }
        }

        // Graceful fallback: synthesise a best-effort answer instead of a hard error.
        messages.push(AgentMessage::User(
            "You have reached the maximum number of reasoning steps. \
             Synthesise the best possible answer from the information gathered so far."
                .to_string(),
        ));
        match self.llm_client.complete_with_tools(&messages, &[]).await? {
            LlmToolResponse::Content(answer) => Ok((answer, messages)),
            _ => Err(AgentError::MaxIterationsExceeded(
                self.config.max_iterations,
            )),
        }
    }

    async fn persist_turn(
        &self,
        conversation_id: ConversationId,
        user_message: &str,
        answer: &str,
        react_messages: &[AgentMessage],
    ) {
        let user_msg = Message::new(conversation_id, MessageRole::User, user_message.to_string());
        if let Err(e) = self.conversation_repository.append_message(&user_msg).await {
            tracing::warn!(error = %e, "Failed to persist agent user message");
        }

        for agent_msg in react_messages {
            match agent_msg {
                AgentMessage::Assistant {
                    content: None,
                    tool_calls,
                } if !tool_calls.is_empty() => {
                    let content =
                        serde_json::to_string(tool_calls).unwrap_or_else(|_| "[]".to_string());
                    let first_name = tool_calls[0].name.as_str();
                    let msg =
                        Message::new_tool_call(conversation_id, ToolName::new(first_name), content);
                    if let Err(e) = self.conversation_repository.append_message(&msg).await {
                        tracing::warn!(error = %e, "Failed to persist agent tool-call message");
                    }
                }
                AgentMessage::ToolResult(result) => {
                    let msg = Message::new_tool_response(
                        conversation_id,
                        result.tool_call_id.clone(),
                        result.tool_name.clone(),
                        result.content.clone(),
                    );
                    if let Err(e) = self.conversation_repository.append_message(&msg).await {
                        tracing::warn!(error = %e, "Failed to persist agent tool-response message");
                    }
                }
                _ => {}
            }
        }

        let assistant_msg =
            Message::new(conversation_id, MessageRole::Assistant, answer.to_string());
        if let Err(e) = self
            .conversation_repository
            .append_message(&assistant_msg)
            .await
        {
            tracing::warn!(error = %e, "Failed to persist agent assistant message");
        }
    }

    fn fire_and_forget_eval(&self, question: &str, answer: &str, correlation_id: Option<String>) {
        if let (Some(event_repo), Some(outbox_repo)) =
            (&self.eval_event_repository, &self.eval_outbox_repository)
        {
            let sources = self
                .rag_source_collector
                .as_ref()
                .map(|c| c.drain())
                .unwrap_or_default();

            let eval_event = EvalEvent::new_agentic(
                question,
                answer,
                sources,
                &self.config.model_config,
                correlation_id,
            );
            let event_repo = Arc::clone(event_repo);
            let outbox_repo = Arc::clone(outbox_repo);
            let span = tracing::Span::current();
            tokio::spawn(
                async move {
                    match event_repo.record(&eval_event).await {
                        Ok(_) => {
                            if let Err(e) = outbox_repo.enqueue(eval_event.id).await {
                                tracing::warn!(error = %e, "Failed to enqueue agent eval outbox");
                            }
                        }
                        Err(e) => tracing::warn!(error = %e, "Failed to record agent eval event"),
                    }
                }
                .instrument(span),
            );
        }
    }

    /// Performs a critic pass on the candidate answer and, if the score falls
    /// below the configured threshold and the budget allows, appends the critic's
    /// feedback and runs one correction iteration.
    ///
    /// The critic receives the full conversation context (user question, tool
    /// results, and candidate answer) so it can judge grounding and completeness.
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

        let mut current_answer = candidate_answer;
        let tools = self.tool_registry.list_tools();
        let timeout_dur = Duration::from_secs(self.config.tool_timeout_secs);

        // Budget for tool-call iterations within a single correction pass.
        const MAX_CORRECTION_TOOL_ITERATIONS: usize = 3;

        for _ in 0..cfg.correction_budget {
            let critic_prompt =
                build_critic_prompt(&messages, &cfg.critic_system_prompt, &current_answer);

            let raw = match self.llm_client.complete(&critic_prompt, "").await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "Critic LLM call failed; skipping reflection");
                    return Ok((current_answer, messages));
                }
            };

            let (score, issues) = parse_critic_response(&raw);
            let needs_correction = score < cfg.score_threshold;

            let _ = progress_tx.try_send(AgentProgressEvent::Reflection {
                score,
                needs_correction,
                issues: issues.clone(),
            });

            if !needs_correction {
                return Ok((current_answer, messages));
            }

            let feedback = format!(
                "Your previous answer scored {score:.2}/1.0 for completeness and grounding. \
                 Issues noted:\n{}\n\nPlease provide a corrected, more complete answer.",
                issues.join(", ")
            );
            messages.push(AgentMessage::User(feedback));

            // The correction pass may need tools (e.g. re-query RAG). Run a
            // bounded mini-loop so we don't bail on the first ToolCalls response.
            for _ in 0..MAX_CORRECTION_TOOL_ITERATIONS {
                match self
                    .llm_client
                    .complete_with_tools(&messages, &tools)
                    .await?
                {
                    LlmToolResponse::Content(r) => {
                        current_answer = r;
                        break;
                    }
                    LlmToolResponse::ToolCalls(calls) => {
                        messages.push(AgentMessage::Assistant {
                            content: None,
                            tool_calls: calls.clone(),
                        });

                        let outcomes: Vec<_> = join_all(calls.iter().map(|call| {
                            tokio::time::timeout(timeout_dur, self.mcp_client.call_tool(call))
                        }))
                        .await;

                        for (call, outcome) in calls.iter().zip(outcomes) {
                            let tool_result = match outcome {
                                Ok(Ok(r)) => r,
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
                            messages.push(AgentMessage::ToolResult(tool_result));
                        }
                    }
                }
            }

            let _ = progress_tx.try_send(AgentProgressEvent::CorrectionApplied);
        }

        Ok((current_answer, messages))
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

        self.persist_turn(
            conversation_id,
            &request.user_message,
            &answer,
            &final_messages,
        )
        .await;

        self.fire_and_forget_eval(
            &request.user_message,
            &answer,
            request.correlation_id.clone(),
        );

        // Fake-stream the already-buffered answer by splitting on whitespace.
        // This avoids a redundant LLM call (Option A): the ReAct loop already
        // produced the full answer via complete_with_tools; re-calling
        // complete_stream_with_messages would duplicate cost and latency.
        let token_stream: LlmTokenStream = {
            let words: Vec<Result<String, LlmClientError>> = answer
                .split_inclusive(' ')
                .map(|w| Ok(w.to_string()))
                .collect();
            Box::pin(futures::stream::iter(words))
        };

        Ok(AgentChatResponse {
            progress_rx,
            token_stream,
            conversation_id,
        })
    }
}
