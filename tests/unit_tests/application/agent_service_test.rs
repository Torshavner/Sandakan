// @AI-BYPASS-LENGTH: all mocks and tests for AgentService live in one place to keep
// mock wiring transparent; splitting would obscure the test intent.
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use sandakan::application::ports::{
    AgentMessage, ConversationRepository, LlmClient, LlmClientError, LlmTokenStream,
    LlmToolResponse, McpClientPort, McpError, RagSourceCollector, RepositoryError, ToolRegistry,
    ToolSchema,
};
use sandakan::application::services::{
    AgentChatRequest, AgentError, AgentProgressEvent, AgentService, AgentServicePort,
};
use sandakan::domain::{
    Conversation, ConversationId, EvalSource, Message, ToolCall, ToolCallId, ToolName, ToolResult,
};
use sandakan::presentation::config::{AgentServiceConfig, ReflectionSettings};
// ─── Mock: LLM returns ToolCalls on first call, Content on second ─────────────

struct MockLlmToolThenContent {
    call_count: AtomicU32,
}

impl MockLlmToolThenContent {
    fn new() -> Self {
        Self {
            call_count: AtomicU32::new(0),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for MockLlmToolThenContent {
    async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
        Ok(String::new())
    }
    async fn complete_stream(&self, _: &str, _: &str) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::once(async {
            Ok("Final agent answer".to_string())
        })))
    }
    async fn complete_with_tools(
        &self,
        _messages: &[AgentMessage],
        _tools: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        if count == 0 {
            Ok(LlmToolResponse::ToolCalls(vec![ToolCall {
                id: ToolCallId::new("call_001"),
                name: ToolName::new("web_search"),
                arguments: serde_json::json!({"query": "test"}),
            }]))
        } else {
            Ok(LlmToolResponse::Content("Final agent answer".to_string()))
        }
    }
}

// ─── Mock: LLM always returns ToolCalls (triggers max iterations) ─────────────

struct MockLlmAlwaysToolCall;

#[async_trait::async_trait]
impl LlmClient for MockLlmAlwaysToolCall {
    async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
        Ok(String::new())
    }
    async fn complete_stream(&self, _: &str, _: &str) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_with_tools(
        &self,
        _: &[AgentMessage],
        _: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        Ok(LlmToolResponse::ToolCalls(vec![ToolCall {
            id: ToolCallId::new("call_loop"),
            name: ToolName::new("web_search"),
            arguments: serde_json::json!({"query": "loop"}),
        }]))
    }
}

// ─── Mock: LLM immediately returns Content ────────────────────────────────────

struct MockLlmImmediateContent;

#[async_trait::async_trait]
impl LlmClient for MockLlmImmediateContent {
    async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
        Ok(String::new())
    }
    async fn complete_stream(&self, _: &str, _: &str) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::once(async {
            Ok("Direct answer".to_string())
        })))
    }
    async fn complete_with_tools(
        &self,
        _: &[AgentMessage],
        _: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        Ok(LlmToolResponse::Content("Direct answer".to_string()))
    }
}

// ─── Mock: MCP client always succeeds ────────────────────────────────────────

struct MockMcpSuccess;

#[async_trait::async_trait]
impl McpClientPort for MockMcpSuccess {
    async fn call_tool(&self, call: &ToolCall) -> Result<ToolResult, McpError> {
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            content: "Search results".to_string(),
        })
    }
}

// ─── Mock: MCP client always fails ───────────────────────────────────────────

struct MockMcpFailing;

#[async_trait::async_trait]
impl McpClientPort for MockMcpFailing {
    async fn call_tool(&self, _call: &ToolCall) -> Result<ToolResult, McpError> {
        Err(McpError::ExecutionFailed("network error".to_string()))
    }
}

// ─── Mock: empty tool registry ───────────────────────────────────────────────

struct MockToolRegistry;

#[async_trait::async_trait]
impl ToolRegistry for MockToolRegistry {
    fn list_tools(&self) -> Vec<ToolSchema> {
        Vec::new()
    }
}

// ─── Mock: in-memory conversation repository ─────────────────────────────────

struct MockConversationRepository;

#[async_trait::async_trait]
impl ConversationRepository for MockConversationRepository {
    async fn create_conversation(
        &self,
        _conversation: &Conversation,
    ) -> Result<(), RepositoryError> {
        Ok(())
    }

    async fn get_conversation(
        &self,
        _id: ConversationId,
    ) -> Result<Option<sandakan::domain::Conversation>, RepositoryError> {
        Ok(None)
    }

    async fn append_message(&self, _message: &Message) -> Result<(), RepositoryError> {
        Ok(())
    }

    async fn get_messages(
        &self,
        _conversation_id: ConversationId,
        _limit: usize,
    ) -> Result<Vec<Message>, RepositoryError> {
        Ok(Vec::new())
    }
}

// ─── Helper ───────────────────────────────────────────────────────────────────

fn default_config() -> AgentServiceConfig {
    AgentServiceConfig {
        model_config: "test/model".to_string(),
        max_iterations: 3,
        tool_timeout_secs: 30,
        tool_fail_fast: false,
        system_prompt: "You are a test agent.".to_string(),
        reflection: ReflectionSettings::default(),
        max_tool_results: 10,
        dynamic_tools_description: false,
        max_context_tokens: 42_000,
        smart_pruning: false,
    }
}

fn reflection_config(
    enabled: bool,
    score_threshold: f32,
    correction_budget: usize,
) -> AgentServiceConfig {
    AgentServiceConfig {
        model_config: "test/model".to_string(),
        max_iterations: 3,
        tool_timeout_secs: 30,
        tool_fail_fast: false,
        system_prompt: "You are a test agent.".to_string(),
        reflection: ReflectionSettings {
            enabled,
            score_threshold,
            correction_budget,
            critic_system_prompt: "You are a critic.".to_string(),
        },
        max_tool_results: 10,
        dynamic_tools_description: false,
        max_context_tokens: 42_000,
        smart_pruning: false,
    }
}

fn build_service(llm: Arc<dyn LlmClient>, mcp: Arc<dyn McpClientPort>) -> AgentService {
    AgentService::new(
        llm,
        mcp,
        Arc::new(MockToolRegistry),
        Arc::new(MockConversationRepository),
        None,
        None,
        None,
        default_config(),
    )
}

fn build_service_with_config(
    llm: Arc<dyn LlmClient>,
    mcp: Arc<dyn McpClientPort>,
    config: AgentServiceConfig,
) -> AgentService {
    AgentService::new(
        llm,
        mcp,
        Arc::new(MockToolRegistry),
        Arc::new(MockConversationRepository),
        None,
        None,
        None,
        config,
    )
}

fn build_service_with_collector(
    llm: Arc<dyn LlmClient>,
    mcp: Arc<dyn McpClientPort>,
    collector: Arc<dyn RagSourceCollector>,
) -> AgentService {
    AgentService::new(
        llm,
        mcp,
        Arc::new(MockToolRegistry),
        Arc::new(MockConversationRepository),
        None,
        None,
        Some(collector),
        default_config(),
    )
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn given_llm_returns_tool_call_then_content_when_chatting_then_react_loop_executes_and_returns_response()
 {
    let llm = Arc::new(MockLlmToolThenContent::new());
    let mcp = Arc::new(MockMcpSuccess);
    let service = build_service(llm, mcp);

    let request = AgentChatRequest {
        conversation_id: None,
        user_message: "What is the news?".to_string(),
        correlation_id: None,
    };

    let result = service.chat(request).await;
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

    let mut response = result.unwrap();

    // Drain the token stream to get the final answer.
    let mut collected = String::new();
    while let Some(token) = futures::StreamExt::next(&mut response.token_stream).await {
        collected.push_str(&token.unwrap());
    }
    assert_eq!(collected, "Final agent answer");
}

#[tokio::test]
async fn given_llm_always_returns_tool_calls_when_chatting_then_returns_max_iterations_exceeded_error()
 {
    let llm = Arc::new(MockLlmAlwaysToolCall);
    let mcp = Arc::new(MockMcpSuccess);
    let service = build_service(llm, mcp);

    let request = AgentChatRequest {
        conversation_id: None,
        user_message: "Loop forever".to_string(),
        correlation_id: None,
    };

    let result = service.chat(request).await;
    let err = result.err();
    assert!(
        matches!(err, Some(AgentError::MaxIterationsExceeded(3))),
        "Expected MaxIterationsExceeded(3), got: {:?}",
        err
    );
}

#[tokio::test]
async fn given_mcp_client_fails_when_fail_fast_true_then_agent_returns_hard_tool_error() {
    let llm = Arc::new(MockLlmToolThenContent::new());
    let mcp = Arc::new(MockMcpFailing);
    let service = build_service_with_config(
        llm,
        mcp,
        AgentServiceConfig {
            tool_fail_fast: true,
            ..default_config()
        },
    );

    let result = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "Search something".to_string(),
            correlation_id: None,
        })
        .await;
    let err = result.err();
    assert!(
        matches!(err, Some(AgentError::Tool(_))),
        "Expected Tool error, got: {:?}",
        err
    );
}

#[tokio::test]
async fn given_llm_returns_content_immediately_when_chatting_then_no_progress_events_in_channel() {
    let llm = Arc::new(MockLlmImmediateContent);
    let mcp = Arc::new(MockMcpSuccess);
    let service = build_service(llm, mcp);

    let request = AgentChatRequest {
        conversation_id: None,
        user_message: "Simple question".to_string(),
        correlation_id: None,
    };

    let result = service.chat(request).await.unwrap();

    // Only a Thinking event should be present (no ToolCall/ToolResult events).
    // The sender is dropped before returning, so try_recv drains the full buffer.
    let mut progress_rx = result.progress_rx;
    let mut events = Vec::new();
    while let Ok(evt) = progress_rx.try_recv() {
        events.push(evt);
    }

    // Should have exactly one Thinking event (iteration 0), no ToolCall/ToolResult.
    assert_eq!(
        events.len(),
        1,
        "Expected 1 Thinking event, got: {}",
        events.len()
    );
    assert!(
        matches!(
            events[0],
            sandakan::application::services::AgentProgressEvent::Thinking { iteration: 0 }
        ),
        "Expected Thinking {{ iteration: 0 }}"
    );
}

// ─── Mock: LLM returns two tool calls in one response, then Content ───────────

struct MockLlmTwoParallelToolsThenContent {
    call_count: AtomicU32,
}

impl MockLlmTwoParallelToolsThenContent {
    fn new() -> Self {
        Self {
            call_count: AtomicU32::new(0),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for MockLlmTwoParallelToolsThenContent {
    async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
        Ok(String::new())
    }
    async fn complete_stream(&self, _: &str, _: &str) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::iter(vec![
            Ok("parallel ".to_string()),
            Ok("answer".to_string()),
        ])))
    }
    async fn complete_with_tools(
        &self,
        _: &[AgentMessage],
        _: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        if count == 0 {
            Ok(LlmToolResponse::ToolCalls(vec![
                ToolCall {
                    id: ToolCallId::new("call_a"),
                    name: ToolName::new("search"),
                    arguments: serde_json::json!({"query": "alpha"}),
                },
                ToolCall {
                    id: ToolCallId::new("call_b"),
                    name: ToolName::new("lookup"),
                    arguments: serde_json::json!({"key": "beta"}),
                },
            ]))
        } else {
            Ok(LlmToolResponse::Content("parallel answer".to_string()))
        }
    }
}

// ─── Mock: MCP records execution order via atomic counter ────────────────────

struct MockMcpOrderRecorder {
    order: Arc<std::sync::Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl McpClientPort for MockMcpOrderRecorder {
    async fn call_tool(&self, call: &ToolCall) -> Result<ToolResult, McpError> {
        self.order.lock().unwrap().push(call.name.to_string());
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            content: format!("result for {}", call.name),
        })
    }
}

#[tokio::test]
async fn given_llm_returns_two_tool_calls_when_executing_then_both_results_appear_in_history() {
    let order = Arc::new(std::sync::Mutex::new(Vec::new()));
    let llm = Arc::new(MockLlmTwoParallelToolsThenContent::new());
    let mcp = Arc::new(MockMcpOrderRecorder {
        order: Arc::clone(&order),
    });
    let service = build_service(llm, mcp);

    let result = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "Do two things".to_string(),
            correlation_id: None,
        })
        .await
        .expect("chat should succeed");

    // Both tool calls were dispatched and their names recorded.
    let executed = order.lock().unwrap().clone();
    assert_eq!(executed.len(), 2, "both tools should have been called");
    assert!(executed.contains(&"search".to_string()));
    assert!(executed.contains(&"lookup".to_string()));

    // Token stream should contain the final answer from complete_stream_with_messages.
    let mut token_stream = result.token_stream;
    let mut collected = String::new();
    while let Some(tok) = futures::StreamExt::next(&mut token_stream).await {
        collected.push_str(&tok.unwrap());
    }
    assert_eq!(collected, "parallel answer");
}

#[tokio::test]
async fn given_one_of_two_parallel_tool_calls_fails_when_fail_fast_true_then_agent_returns_tool_error()
 {
    // MCP that fails for "lookup" but succeeds for "search".
    struct MockMcpPartialFail;
    #[async_trait::async_trait]
    impl McpClientPort for MockMcpPartialFail {
        async fn call_tool(&self, call: &ToolCall) -> Result<ToolResult, McpError> {
            if call.name.as_str() == "lookup" {
                Err(McpError::ExecutionFailed("lookup failed".to_string()))
            } else {
                Ok(ToolResult {
                    tool_call_id: call.id.clone(),
                    tool_name: call.name.clone(),
                    content: "ok".to_string(),
                })
            }
        }
    }

    let llm = Arc::new(MockLlmTwoParallelToolsThenContent::new());
    let mcp = Arc::new(MockMcpPartialFail);
    // fail_fast = true: the first failing tool aborts the turn.
    let service = build_service_with_config(
        llm,
        mcp,
        AgentServiceConfig {
            tool_fail_fast: true,
            ..default_config()
        },
    );

    let result = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "Do two things, one will fail".to_string(),
            correlation_id: None,
        })
        .await;

    assert!(
        matches!(result, Err(AgentError::Tool(_))),
        "expected Err(Tool(_))"
    );
}

// ─── Mock: RagSourceCollector that records collected sources ──────────────────

struct MockRagSourceCollector {
    collected: Arc<std::sync::Mutex<Vec<EvalSource>>>,
}

impl MockRagSourceCollector {
    fn new() -> Self {
        Self {
            collected: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

impl RagSourceCollector for MockRagSourceCollector {
    fn collect(&self, mut sources: Vec<EvalSource>) {
        self.collected.lock().unwrap().append(&mut sources);
    }

    fn drain(&self) -> Vec<EvalSource> {
        std::mem::take(&mut self.collected.lock().unwrap())
    }
}

// ─── Mock: MCP that simulates rag_search populating the side-channel ─────────

struct MockMcpRagSearch {
    collector: Arc<dyn RagSourceCollector>,
}

#[async_trait::async_trait]
impl McpClientPort for MockMcpRagSearch {
    async fn call_tool(&self, call: &ToolCall) -> Result<ToolResult, McpError> {
        if call.name.as_str() == "rag_search" {
            // Simulate what RagSearchAdapter does: populate the side-channel.
            self.collector.collect(vec![EvalSource {
                text: "retrieved chunk text".to_string(),
                page: Some(1),
                score: 0.9,
            }]);
        }
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            content: "Found 1 relevant sources:\n1. [Page 1, score: 0.90]: retrieved chunk text"
                .to_string(),
        })
    }
}

// ─── Mock: LLM that issues a rag_search call, then returns content ────────────

struct MockLlmRagThenContent {
    call_count: AtomicU32,
}

impl MockLlmRagThenContent {
    fn new() -> Self {
        Self {
            call_count: AtomicU32::new(0),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for MockLlmRagThenContent {
    async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
        Ok(String::new())
    }
    async fn complete_stream(&self, _: &str, _: &str) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::once(async {
            Ok("RAG-based answer".to_string())
        })))
    }
    async fn complete_with_tools(
        &self,
        _: &[AgentMessage],
        _: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        if count == 0 {
            Ok(LlmToolResponse::ToolCalls(vec![ToolCall {
                id: ToolCallId::new("call_rag"),
                name: ToolName::new("rag_search"),
                arguments: serde_json::json!({"query": "test question"}),
            }]))
        } else {
            Ok(LlmToolResponse::Content("RAG-based answer".to_string()))
        }
    }
}

#[tokio::test]
async fn given_rag_tool_collects_sources_when_eval_enabled_then_eval_event_has_non_empty_sources() {
    // The collector is shared between the mock MCP (writer) and the service (reader).
    // With eval repos wired as None, fire_and_forget_eval is a no-op, so we verify
    // the collector was populated during tool execution by inspecting it afterwards.
    let collector = Arc::new(MockRagSourceCollector::new());
    let mcp = Arc::new(MockMcpRagSearch {
        collector: Arc::clone(&collector) as Arc<dyn RagSourceCollector>,
    });
    let service = build_service_with_collector(
        Arc::new(MockLlmRagThenContent::new()),
        mcp,
        Arc::clone(&collector) as Arc<dyn RagSourceCollector>,
    );

    let result = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "What does the KB say?".to_string(),
            correlation_id: None,
        })
        .await
        .expect("chat should succeed");

    // Sources accumulated during the rag_search tool call.
    let sources = collector.drain();
    assert_eq!(sources.len(), 1, "collector should hold the RAG source");
    assert_eq!(sources[0].text, "retrieved chunk text");

    // Token stream still works normally.
    let mut token_stream = result.token_stream;
    let mut collected = String::new();
    while let Some(tok) = futures::StreamExt::next(&mut token_stream).await {
        collected.push_str(&tok.unwrap());
    }
    assert_eq!(collected, "RAG-based answer");
}

#[tokio::test]
async fn given_no_rag_tool_invoked_when_eval_fires_then_eval_event_has_empty_sources() {
    let collector = Arc::new(MockRagSourceCollector::new());
    let service = build_service_with_collector(
        Arc::new(MockLlmImmediateContent),
        Arc::new(MockMcpSuccess),
        Arc::clone(&collector) as Arc<dyn RagSourceCollector>,
    );

    service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "No RAG needed".to_string(),
            correlation_id: None,
        })
        .await
        .expect("chat should succeed");

    // No rag_search was called, so collector should remain empty.
    let remaining = collector.drain();
    assert!(
        remaining.is_empty(),
        "no sources should be collected when rag_search was not invoked"
    );
}

#[tokio::test]
async fn given_llm_returns_content_when_chatting_then_token_stream_yields_real_tokens() {
    let llm = Arc::new(MockLlmImmediateContent);
    let mcp = Arc::new(MockMcpSuccess);
    let service = build_service(llm, mcp);

    let mut response = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "Stream me an answer".to_string(),
            correlation_id: None,
        })
        .await
        .expect("chat should succeed");

    let mut tokens: Vec<String> = Vec::new();
    while let Some(tok) = futures::StreamExt::next(&mut response.token_stream).await {
        tokens.push(tok.unwrap());
    }

    // The mock streams one token; the important invariant is that it is
    // non-empty and carries real content (not a single-chunk buffered string).
    assert!(!tokens.is_empty(), "token stream should not be empty");
    let full = tokens.join("");
    assert_eq!(full, "Direct answer");
}

// ─── US-027: soft-fail and timeout tests ─────────────────────────────────────

#[tokio::test]
async fn given_tool_fails_with_execution_error_when_fail_fast_false_then_agent_continues_and_returns_content()
 {
    // fail_fast = false (default): error becomes a [tool_error] ToolResult and
    // the LLM gets a second chance to produce a Content response.
    let llm = Arc::new(MockLlmToolThenContent::new());
    let mcp = Arc::new(MockMcpFailing);
    let service = build_service_with_config(
        llm,
        mcp,
        AgentServiceConfig {
            tool_fail_fast: false,
            ..default_config()
        },
    );

    let result = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "Search something".to_string(),
            correlation_id: None,
        })
        .await;

    assert!(
        result.is_ok(),
        "Expected Ok when fail_fast=false, got: {:?}",
        result.err()
    );

    let mut response = result.unwrap();
    let mut collected = String::new();
    while let Some(tok) = futures::StreamExt::next(&mut response.token_stream).await {
        collected.push_str(&tok.unwrap());
    }
    assert_eq!(collected, "Final agent answer");
}

#[tokio::test]
async fn given_tool_exceeds_timeout_when_fail_fast_false_then_agent_sees_timeout_marker_in_tool_result()
 {
    // A mock MCP that hangs for longer than the configured timeout.
    struct MockMcpHanging;
    #[async_trait::async_trait]
    impl McpClientPort for MockMcpHanging {
        async fn call_tool(&self, _call: &ToolCall) -> Result<ToolResult, McpError> {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            Err(McpError::ExecutionFailed("never".to_string()))
        }
    }

    // A mock LLM that checks for the [tool_timeout] marker in the second call.
    struct MockLlmAssertTimeoutMarker {
        call_count: std::sync::atomic::AtomicU32,
    }
    #[async_trait::async_trait]
    impl LlmClient for MockLlmAssertTimeoutMarker {
        async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
            Ok(String::new())
        }
        async fn complete_stream(
            &self,
            _: &str,
            _: &str,
        ) -> Result<LlmTokenStream, LlmClientError> {
            Ok(Box::pin(futures::stream::empty()))
        }
        async fn complete_stream_with_messages(
            &self,
            _: &[AgentMessage],
        ) -> Result<LlmTokenStream, LlmClientError> {
            Ok(Box::pin(futures::stream::once(async {
                Ok("recovered answer".to_string())
            })))
        }
        async fn complete_with_tools(
            &self,
            messages: &[AgentMessage],
            _: &[ToolSchema],
        ) -> Result<LlmToolResponse, LlmClientError> {
            let count = self
                .call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count == 0 {
                Ok(LlmToolResponse::ToolCalls(vec![ToolCall {
                    id: ToolCallId::new("call_slow"),
                    name: ToolName::new("slow_tool"),
                    arguments: serde_json::json!({}),
                }]))
            } else {
                // Verify the [tool_timeout] marker reached the LLM context.
                let has_marker = messages.iter().any(|m| {
                    if let AgentMessage::ToolResult(r) = m {
                        r.content.starts_with("[tool_timeout]")
                    } else {
                        false
                    }
                });
                assert!(has_marker, "LLM context must contain [tool_timeout] marker");
                Ok(LlmToolResponse::Content("recovered answer".to_string()))
            }
        }
    }

    let llm = Arc::new(MockLlmAssertTimeoutMarker {
        call_count: std::sync::atomic::AtomicU32::new(0),
    });
    let mcp = Arc::new(MockMcpHanging);
    let service = build_service_with_config(
        llm,
        mcp,
        AgentServiceConfig {
            tool_timeout_secs: 1, // short timeout so the test is fast
            tool_fail_fast: false,
            ..default_config()
        },
    );

    let result = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "Call a slow tool".to_string(),
            correlation_id: None,
        })
        .await;

    assert!(
        result.is_ok(),
        "Expected Ok after timeout soft-fail, got: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn given_tool_not_found_when_fail_fast_false_then_agent_still_returns_hard_error() {
    struct MockMcpToolNotFound;
    #[async_trait::async_trait]
    impl McpClientPort for MockMcpToolNotFound {
        async fn call_tool(&self, call: &ToolCall) -> Result<ToolResult, McpError> {
            Err(McpError::ToolNotFound(call.name.to_string()))
        }
    }

    let llm = Arc::new(MockLlmToolThenContent::new());
    let mcp = Arc::new(MockMcpToolNotFound);
    // fail_fast is false — ToolNotFound must still hard-fail regardless.
    let service = build_service_with_config(
        llm,
        mcp,
        AgentServiceConfig {
            tool_fail_fast: false,
            ..default_config()
        },
    );

    let result = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "Use missing tool".to_string(),
            correlation_id: None,
        })
        .await;

    let err = result.err();
    assert!(
        matches!(err, Some(AgentError::Tool(_))),
        "ToolNotFound must always hard-fail, got: {:?}",
        err
    );
}

#[tokio::test]
async fn given_one_of_two_parallel_tools_fails_when_fail_fast_false_then_other_result_included_in_history()
 {
    // MCP: "search" succeeds, "lookup" fails with ExecutionFailed.
    struct MockMcpOneFailOneSuccess;
    #[async_trait::async_trait]
    impl McpClientPort for MockMcpOneFailOneSuccess {
        async fn call_tool(&self, call: &ToolCall) -> Result<ToolResult, McpError> {
            if call.name.as_str() == "lookup" {
                Err(McpError::ExecutionFailed("lookup down".to_string()))
            } else {
                Ok(ToolResult {
                    tool_call_id: call.id.clone(),
                    tool_name: call.name.clone(),
                    content: "search ok".to_string(),
                })
            }
        }
    }

    // LLM that verifies both ToolResult messages are in history on the second call.
    struct MockLlmAssertBothResults {
        call_count: std::sync::atomic::AtomicU32,
    }
    #[async_trait::async_trait]
    impl LlmClient for MockLlmAssertBothResults {
        async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
            Ok(String::new())
        }
        async fn complete_stream(
            &self,
            _: &str,
            _: &str,
        ) -> Result<LlmTokenStream, LlmClientError> {
            Ok(Box::pin(futures::stream::empty()))
        }
        async fn complete_stream_with_messages(
            &self,
            _: &[AgentMessage],
        ) -> Result<LlmTokenStream, LlmClientError> {
            Ok(Box::pin(futures::stream::once(async {
                Ok("parallel soft-fail answer".to_string())
            })))
        }
        async fn complete_with_tools(
            &self,
            messages: &[AgentMessage],
            _: &[ToolSchema],
        ) -> Result<LlmToolResponse, LlmClientError> {
            let count = self
                .call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count == 0 {
                Ok(LlmToolResponse::ToolCalls(vec![
                    ToolCall {
                        id: ToolCallId::new("call_a"),
                        name: ToolName::new("search"),
                        arguments: serde_json::json!({"query": "alpha"}),
                    },
                    ToolCall {
                        id: ToolCallId::new("call_b"),
                        name: ToolName::new("lookup"),
                        arguments: serde_json::json!({"key": "beta"}),
                    },
                ]))
            } else {
                // Both ToolResult messages must be in the history.
                let tool_results: Vec<_> = messages
                    .iter()
                    .filter_map(|m| {
                        if let AgentMessage::ToolResult(r) = m {
                            Some(r.content.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                assert_eq!(
                    tool_results.len(),
                    2,
                    "both tool results must be in history"
                );
                let has_success = tool_results.iter().any(|c| c == "search ok");
                let has_error = tool_results.iter().any(|c| c.starts_with("[tool_error]"));
                assert!(has_success, "successful tool result must be present");
                assert!(
                    has_error,
                    "failed tool result must carry [tool_error] prefix"
                );

                Ok(LlmToolResponse::Content(
                    "parallel soft-fail answer".to_string(),
                ))
            }
        }
    }

    let llm = Arc::new(MockLlmAssertBothResults {
        call_count: std::sync::atomic::AtomicU32::new(0),
    });
    let mcp = Arc::new(MockMcpOneFailOneSuccess);
    let service = build_service_with_config(
        llm,
        mcp,
        AgentServiceConfig {
            tool_fail_fast: false,
            ..default_config()
        },
    );

    let result = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "Two parallel tools, one fails".to_string(),
            correlation_id: None,
        })
        .await;

    assert!(
        result.is_ok(),
        "Expected Ok with soft-fail, got: {:?}",
        result.err()
    );
}

// ─── Reflection tests ─────────────────────────────────────────────────────────

// Mock: LLM returns high critic score → no correction needed.
struct MockLlmHighCriticScore;

#[async_trait::async_trait]
impl LlmClient for MockLlmHighCriticScore {
    async fn complete(&self, _prompt: &str, _ctx: &str) -> Result<String, LlmClientError> {
        // Critic response: score above default threshold of 0.7
        Ok("SCORE: 0.85\nISSUES: none".to_string())
    }
    async fn complete_stream(&self, _: &str, _: &str) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::once(async {
            Ok("High-quality answer".to_string())
        })))
    }
    async fn complete_with_tools(
        &self,
        _: &[AgentMessage],
        _: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        Ok(LlmToolResponse::Content("High-quality answer".to_string()))
    }
}

// Mock: LLM returns low critic score on first `complete` call, then a corrected answer.
struct MockLlmLowCriticScoreThenCorrection {
    complete_call_count: AtomicU32,
    complete_with_tools_count: AtomicU32,
}

impl MockLlmLowCriticScoreThenCorrection {
    fn new() -> Self {
        Self {
            complete_call_count: AtomicU32::new(0),
            complete_with_tools_count: AtomicU32::new(0),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for MockLlmLowCriticScoreThenCorrection {
    async fn complete(&self, _prompt: &str, _ctx: &str) -> Result<String, LlmClientError> {
        self.complete_call_count.fetch_add(1, Ordering::SeqCst);
        // Critic response: score below threshold
        Ok("SCORE: 0.45\nISSUES: incomplete, missing sources".to_string())
    }
    async fn complete_stream(&self, _: &str, _: &str) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::once(async {
            Ok("Refined answer".to_string())
        })))
    }
    async fn complete_with_tools(
        &self,
        _: &[AgentMessage],
        _: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        let count = self
            .complete_with_tools_count
            .fetch_add(1, Ordering::SeqCst);
        if count == 0 {
            // ReAct loop: return candidate answer
            Ok(LlmToolResponse::Content(
                "Weak candidate answer".to_string(),
            ))
        } else {
            // Correction pass: return refined answer
            Ok(LlmToolResponse::Content("Refined answer".to_string()))
        }
    }
}

// Mock: LLM returns unparseable critic response.
struct MockLlmUnparseableCritic;

#[async_trait::async_trait]
impl LlmClient for MockLlmUnparseableCritic {
    async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
        Ok("This is not a valid critic response format at all.".to_string())
    }
    async fn complete_stream(&self, _: &str, _: &str) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::once(async {
            Ok("Original answer".to_string())
        })))
    }
    async fn complete_with_tools(
        &self,
        _: &[AgentMessage],
        _: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        Ok(LlmToolResponse::Content("Original answer".to_string()))
    }
}

#[tokio::test]
async fn given_high_quality_answer_when_reflection_enabled_then_no_correction_pass_runs() {
    let llm = Arc::new(MockLlmHighCriticScore);
    let mcp = Arc::new(MockMcpSuccess);
    let service = build_service_with_config(llm, mcp, reflection_config(true, 0.7, 1));

    let mut response = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "What is Rust?".to_string(),
            correlation_id: None,
        })
        .await
        .expect("chat should succeed");

    // Collect progress events.
    let mut events = Vec::new();
    while let Ok(evt) = response.progress_rx.try_recv() {
        events.push(evt);
    }

    let reflection_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::Reflection { .. }))
        .collect();

    assert_eq!(
        reflection_events.len(),
        1,
        "exactly one Reflection event expected"
    );

    let correction_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::CorrectionApplied))
        .collect();

    assert!(
        correction_events.is_empty(),
        "no CorrectionApplied event expected for high-score answer"
    );

    if let AgentProgressEvent::Reflection {
        score,
        needs_correction,
        ..
    } = reflection_events[0]
    {
        assert!(*score >= 0.7, "score should be above threshold");
        assert!(
            !needs_correction,
            "needs_correction must be false for high score"
        );
    }
}

#[tokio::test]
async fn given_low_score_answer_when_reflection_enabled_then_correction_message_appended_and_refined_answer_returned()
 {
    let llm = Arc::new(MockLlmLowCriticScoreThenCorrection::new());
    let mcp = Arc::new(MockMcpSuccess);
    let service = build_service_with_config(llm, mcp, reflection_config(true, 0.7, 1));

    let mut response = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "Explain something complex".to_string(),
            correlation_id: None,
        })
        .await
        .expect("chat should succeed");

    let mut events = Vec::new();
    while let Ok(evt) = response.progress_rx.try_recv() {
        events.push(evt);
    }

    let reflection_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::Reflection { .. }))
        .collect();

    assert_eq!(reflection_events.len(), 1);

    let correction_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::CorrectionApplied))
        .collect();

    assert_eq!(
        correction_events.len(),
        1,
        "CorrectionApplied event must be emitted after low-score answer"
    );

    if let AgentProgressEvent::Reflection {
        score,
        needs_correction,
        issues,
    } = reflection_events[0]
    {
        assert!(*score < 0.7, "score should be below threshold");
        assert!(
            needs_correction,
            "needs_correction must be true for low score"
        );
        assert!(!issues.is_empty(), "issues must be non-empty");
    }
}

#[tokio::test]
async fn given_correction_budget_zero_when_score_below_threshold_then_original_answer_returned() {
    let llm = Arc::new(MockLlmLowCriticScoreThenCorrection::new());
    let mcp = Arc::new(MockMcpSuccess);
    // Budget = 0: even if score is low, no correction pass runs.
    let service = build_service_with_config(llm, mcp, reflection_config(true, 0.7, 0));

    let mut response = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "Budget is zero".to_string(),
            correlation_id: None,
        })
        .await
        .expect("chat should succeed");

    let mut events = Vec::new();
    while let Ok(evt) = response.progress_rx.try_recv() {
        events.push(evt);
    }

    let correction_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::CorrectionApplied))
        .collect();

    assert!(
        correction_events.is_empty(),
        "no correction should run when budget = 0"
    );

    let reflection_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::Reflection { .. }))
        .collect();

    assert!(
        reflection_events.is_empty(),
        "no Reflection event emitted when budget = 0 (loop never enters)"
    );
}

// Mock: critic always scores below threshold; each correction is also below threshold.
// Tracks how many times `complete` (critic) and `complete_with_tools` (correction) are called.
struct MockLlmAlwaysLowScore {
    critic_call_count: AtomicU32,
    correction_call_count: AtomicU32,
}

impl MockLlmAlwaysLowScore {
    fn new() -> Self {
        Self {
            critic_call_count: AtomicU32::new(0),
            correction_call_count: AtomicU32::new(0),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for MockLlmAlwaysLowScore {
    async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
        self.critic_call_count.fetch_add(1, Ordering::SeqCst);
        Ok("SCORE: 0.3\nISSUES: still incomplete".to_string())
    }
    async fn complete_stream(&self, _: &str, _: &str) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::once(async {
            Ok("still weak".to_string())
        })))
    }
    async fn complete_with_tools(
        &self,
        _: &[AgentMessage],
        _: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        let count = self.correction_call_count.fetch_add(1, Ordering::SeqCst);
        if count == 0 {
            Ok(LlmToolResponse::Content("initial weak answer".to_string()))
        } else {
            Ok(LlmToolResponse::Content("still weak answer".to_string()))
        }
    }
}

// Mock: critic returns low score on iterations 1 and 2, then passes on iteration 3.
// Verifies that the loop exits early once the threshold is reached.
struct MockLlmLowThenPassOnThirdCritic {
    critic_call_count: AtomicU32,
    correction_call_count: AtomicU32,
}

impl MockLlmLowThenPassOnThirdCritic {
    fn new() -> Self {
        Self {
            critic_call_count: AtomicU32::new(0),
            correction_call_count: AtomicU32::new(0),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for MockLlmLowThenPassOnThirdCritic {
    async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
        let n = self.critic_call_count.fetch_add(1, Ordering::SeqCst);
        if n < 2 {
            Ok("SCORE: 0.4\nISSUES: incomplete".to_string())
        } else {
            Ok("SCORE: 0.9\nISSUES: none".to_string())
        }
    }
    async fn complete_stream(&self, _: &str, _: &str) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::once(async {
            Ok("refined answer".to_string())
        })))
    }
    async fn complete_with_tools(
        &self,
        _: &[AgentMessage],
        _: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        let n = self.correction_call_count.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            Ok(LlmToolResponse::Content("initial answer".to_string()))
        } else {
            Ok(LlmToolResponse::Content("corrected answer".to_string()))
        }
    }
}

#[tokio::test]
async fn given_correction_budget_3_when_score_always_low_then_all_three_corrections_run() {
    let llm = Arc::new(MockLlmAlwaysLowScore::new());
    let llm_clone = llm.clone();
    let mcp = Arc::new(MockMcpSuccess);
    let service = build_service_with_config(llm, mcp, reflection_config(true, 0.7, 3));

    let mut response = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "explain something".to_string(),
            correlation_id: None,
        })
        .await
        .expect("chat should succeed");

    let mut events = Vec::new();
    while let Ok(evt) = response.progress_rx.try_recv() {
        events.push(evt);
    }

    let reflection_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::Reflection { .. }))
        .collect();

    let correction_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::CorrectionApplied))
        .collect();

    assert_eq!(
        reflection_events.len(),
        3,
        "one Reflection event per budget iteration"
    );
    assert_eq!(
        correction_events.len(),
        3,
        "CorrectionApplied emitted for every budget iteration when score stays low"
    );
    assert_eq!(
        llm_clone.critic_call_count.load(Ordering::SeqCst),
        3,
        "critic LLM called exactly correction_budget times"
    );
}

#[tokio::test]
async fn given_correction_budget_5_when_second_correction_passes_threshold_then_loop_stops_early() {
    let llm = Arc::new(MockLlmLowThenPassOnThirdCritic::new());
    let llm_clone = llm.clone();
    let mcp = Arc::new(MockMcpSuccess);
    let service = build_service_with_config(llm, mcp, reflection_config(true, 0.7, 5));

    let mut response = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "explain something".to_string(),
            correlation_id: None,
        })
        .await
        .expect("chat should succeed");

    let mut events = Vec::new();
    while let Ok(evt) = response.progress_rx.try_recv() {
        events.push(evt);
    }

    let reflection_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::Reflection { .. }))
        .collect();

    let correction_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::CorrectionApplied))
        .collect();

    assert_eq!(
        reflection_events.len(),
        3,
        "Reflection fires on iterations 1 and 2 (low) then on iteration 3 (passes) — 3 total"
    );
    assert_eq!(
        correction_events.len(),
        2,
        "CorrectionApplied only for iterations where score was below threshold"
    );
    assert_eq!(
        llm_clone.critic_call_count.load(Ordering::SeqCst),
        3,
        "critic stops after first passing score; remaining budget (5-3=2) unused"
    );

    let last_reflection = reflection_events.last().unwrap();
    if let AgentProgressEvent::Reflection {
        score,
        needs_correction,
        ..
    } = last_reflection
    {
        assert!(
            *score >= 0.7,
            "final Reflection event must show passing score"
        );
        assert!(
            !needs_correction,
            "final Reflection event must not request further correction"
        );
    }
}

#[tokio::test]
async fn given_reflection_disabled_when_agent_runs_then_critic_llm_not_called() {
    // With reflection disabled, only complete_with_tools is called (not complete).
    struct MockLlmTrackCompleteCalls {
        complete_call_count: AtomicU32,
    }

    #[async_trait::async_trait]
    impl LlmClient for MockLlmTrackCompleteCalls {
        async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
            self.complete_call_count.fetch_add(1, Ordering::SeqCst);
            Ok("should not be called".to_string())
        }
        async fn complete_stream(
            &self,
            _: &str,
            _: &str,
        ) -> Result<LlmTokenStream, LlmClientError> {
            Ok(Box::pin(futures::stream::empty()))
        }
        async fn complete_stream_with_messages(
            &self,
            _: &[AgentMessage],
        ) -> Result<LlmTokenStream, LlmClientError> {
            Ok(Box::pin(futures::stream::once(async {
                Ok("Direct answer".to_string())
            })))
        }
        async fn complete_with_tools(
            &self,
            _: &[AgentMessage],
            _: &[ToolSchema],
        ) -> Result<LlmToolResponse, LlmClientError> {
            Ok(LlmToolResponse::Content("Direct answer".to_string()))
        }
    }

    let llm = Arc::new(MockLlmTrackCompleteCalls {
        complete_call_count: AtomicU32::new(0),
    });
    let complete_call_count = Arc::clone(&llm);
    let mcp = Arc::new(MockMcpSuccess);

    // Reflection disabled.
    let service = build_service_with_config(llm, mcp, reflection_config(false, 0.7, 1));

    let mut response = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "Simple question".to_string(),
            correlation_id: None,
        })
        .await
        .expect("chat should succeed");

    let mut events = Vec::new();
    while let Ok(evt) = response.progress_rx.try_recv() {
        events.push(evt);
    }

    let reflection_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::Reflection { .. }))
        .collect();

    assert!(
        reflection_events.is_empty(),
        "no Reflection event when reflection is disabled"
    );

    assert_eq!(
        complete_call_count
            .complete_call_count
            .load(Ordering::SeqCst),
        0,
        "critic LLM (complete) must not be called when reflection is disabled"
    );
}

#[tokio::test]
async fn given_critic_returns_unparseable_response_when_reflecting_then_answer_returned_unchanged()
{
    let llm = Arc::new(MockLlmUnparseableCritic);
    let mcp = Arc::new(MockMcpSuccess);
    let service = build_service_with_config(llm, mcp, reflection_config(true, 0.7, 1));

    // Should not error out — graceful degradation treats unparseable as score = 1.0.
    let mut response = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "What does the critic say?".to_string(),
            correlation_id: None,
        })
        .await
        .expect("chat should succeed even with unparseable critic response");

    let mut events = Vec::new();
    while let Ok(evt) = response.progress_rx.try_recv() {
        events.push(evt);
    }

    // Reflection event emitted with score = 1.0 (default), needs_correction = false.
    let reflection_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::Reflection { .. }))
        .collect();

    assert_eq!(reflection_events.len(), 1, "Reflection event emitted");

    if let AgentProgressEvent::Reflection {
        score,
        needs_correction,
        ..
    } = reflection_events[0]
    {
        assert_eq!(*score, 1.0, "unparseable response treated as score = 1.0");
        assert!(!needs_correction, "needs_correction false for score = 1.0");
    }

    let correction_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::CorrectionApplied))
        .collect();

    assert!(
        correction_events.is_empty(),
        "no CorrectionApplied for unparseable critic"
    );
}

// ─── truncate_for_event (via AgentProgressEvent::ToolResult) ──────────────────
//
// The helper is private, so we exercise it indirectly: a tool result whose
// content contains multi-byte UTF-8 sequences must never panic and must be
// correctly truncated at a codepoint boundary.

/// MCP that returns a fixed content string, allowing the test to inject arbitrary bytes.
struct MockMcpWithContent(String);

#[async_trait::async_trait]
impl McpClientPort for MockMcpWithContent {
    async fn call_tool(&self, call: &ToolCall) -> Result<ToolResult, McpError> {
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            content: self.0.clone(),
        })
    }
}

/// LLM that issues one tool call, then returns Content — used by truncation tests.
struct MockLlmOneThenContent;

#[async_trait::async_trait]
impl LlmClient for MockLlmOneThenContent {
    async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
        Ok(String::new())
    }
    async fn complete_stream(&self, _: &str, _: &str) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::once(async {
            Ok("ok".to_string())
        })))
    }
    async fn complete_with_tools(
        &self,
        messages: &[AgentMessage],
        _: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        // First call: emit a tool call. Subsequent calls: return content.
        let already_has_tool_result = messages
            .iter()
            .any(|m| matches!(m, AgentMessage::ToolResult(_)));
        if already_has_tool_result {
            Ok(LlmToolResponse::Content("ok".to_string()))
        } else {
            Ok(LlmToolResponse::ToolCalls(vec![ToolCall {
                id: ToolCallId::new("call_utf8"),
                name: ToolName::new("any_tool"),
                arguments: serde_json::json!({}),
            }]))
        }
    }
}

#[tokio::test]
async fn given_tool_result_with_ascii_content_when_truncating_then_progress_event_emitted_without_panic()
 {
    let content = "a".repeat(200);
    let service = build_service(
        Arc::new(MockLlmOneThenContent),
        Arc::new(MockMcpWithContent(content)),
    );

    let mut response = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "trigger ascii tool".to_string(),
            correlation_id: None,
        })
        .await
        .expect("should not panic on ascii content");

    let mut events = Vec::new();
    while let Ok(evt) = response.progress_rx.try_recv() {
        events.push(evt);
    }

    let tool_result_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::ToolResult { .. }))
        .collect();

    assert!(
        !tool_result_events.is_empty(),
        "ToolResult event must be emitted"
    );
}

#[tokio::test]
async fn given_tool_result_with_multibyte_utf8_when_truncating_then_no_panic_and_event_emitted() {
    // Each '日' is 3 bytes. 50 repetitions = 150 bytes — exceeds the 120-byte
    // truncation limit. The cut at byte 120 would land inside a 3-byte codepoint
    // without the floor_char_boundary fix, causing a panic.
    let content = "日".repeat(50);
    let service = build_service(
        Arc::new(MockLlmOneThenContent),
        Arc::new(MockMcpWithContent(content)),
    );

    let mut response = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "trigger multibyte tool".to_string(),
            correlation_id: None,
        })
        .await
        .expect("must not panic on multibyte UTF-8 content");

    let mut events = Vec::new();
    while let Ok(evt) = response.progress_rx.try_recv() {
        events.push(evt);
    }

    let tool_result_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::ToolResult { .. }))
        .collect();

    assert!(
        !tool_result_events.is_empty(),
        "ToolResult progress event must be emitted"
    );

    if let AgentProgressEvent::ToolResult {
        truncated_content, ..
    } = &tool_result_events[0]
    {
        // Content was longer than 120 bytes so it must have been truncated.
        assert!(
            truncated_content.ends_with('…'),
            "truncated content must end with ellipsis, got: {truncated_content:?}"
        );
        // The truncated prefix must be valid UTF-8 (no partial codepoints).
        assert!(
            std::str::from_utf8(truncated_content.as_bytes()).is_ok(),
            "truncated content must be valid UTF-8"
        );
    }
}

#[tokio::test]
async fn given_tool_result_with_emoji_content_when_truncating_then_no_panic_and_event_emitted() {
    // Each emoji ('🦀') is 4 bytes. 40 repetitions = 160 bytes — exceeds 120.
    // Without floor_char_boundary the cut at byte 120 hits inside a 4-byte sequence.
    let content = "🦀".repeat(40);
    let service = build_service(
        Arc::new(MockLlmOneThenContent),
        Arc::new(MockMcpWithContent(content)),
    );

    let mut response = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "trigger emoji tool".to_string(),
            correlation_id: None,
        })
        .await
        .expect("must not panic on emoji content");

    let mut events = Vec::new();
    while let Ok(evt) = response.progress_rx.try_recv() {
        events.push(evt);
    }

    let tool_result_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, AgentProgressEvent::ToolResult { .. }))
        .collect();

    assert!(
        !tool_result_events.is_empty(),
        "ToolResult progress event must be emitted"
    );

    if let AgentProgressEvent::ToolResult {
        truncated_content, ..
    } = &tool_result_events[0]
    {
        assert!(
            truncated_content.ends_with('…'),
            "truncated content must end with ellipsis, got: {truncated_content:?}"
        );
        assert!(
            std::str::from_utf8(truncated_content.as_bytes()).is_ok(),
            "truncated content must be valid UTF-8"
        );
    }
}

#[tokio::test]
async fn given_tool_result_shorter_than_limit_when_truncating_then_content_returned_unchanged() {
    // Content well below the 120-byte limit — must not be truncated.
    let content = "short result".to_string();
    let service = build_service(
        Arc::new(MockLlmOneThenContent),
        Arc::new(MockMcpWithContent(content.clone())),
    );

    let mut response = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "trigger short tool".to_string(),
            correlation_id: None,
        })
        .await
        .expect("should succeed");

    let mut events = Vec::new();
    while let Ok(evt) = response.progress_rx.try_recv() {
        events.push(evt);
    }

    if let Some(AgentProgressEvent::ToolResult {
        truncated_content, ..
    }) = events
        .iter()
        .find(|e| matches!(e, AgentProgressEvent::ToolResult { .. }))
    {
        assert_eq!(
            truncated_content, &content,
            "short content must not be modified"
        );
    }
}

// ─── System-prompt tests ──────────────────────────────────────────────────────

/// Mock that captures the first message received by `complete_with_tools`.
struct MockLlmCaptureFirstMessage {
    first_message: std::sync::Mutex<Option<AgentMessage>>,
}

impl MockLlmCaptureFirstMessage {
    fn new() -> Self {
        Self {
            first_message: std::sync::Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for MockLlmCaptureFirstMessage {
    async fn complete(&self, _: &str, _: &str) -> Result<String, LlmClientError> {
        Ok(String::new())
    }
    async fn complete_stream(&self, _: &str, _: &str) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::empty()))
    }
    async fn complete_stream_with_messages(
        &self,
        _: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::once(async {
            Ok("answer".to_string())
        })))
    }
    async fn complete_with_tools(
        &self,
        messages: &[AgentMessage],
        _: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        let mut guard = self.first_message.lock().unwrap();
        if guard.is_none() {
            *guard = messages.first().cloned();
        }
        Ok(LlmToolResponse::Content("answer".to_string()))
    }
}

#[tokio::test]
async fn given_configured_system_prompt_when_agent_chat_called_then_system_message_is_first() {
    let expected_prompt = "You are a specialized agent for testing.";
    let llm = Arc::new(MockLlmCaptureFirstMessage::new());
    let llm_clone = Arc::clone(&llm);

    let service = build_service_with_config(
        llm,
        Arc::new(MockMcpSuccess),
        AgentServiceConfig {
            system_prompt: expected_prompt.to_string(),
            ..default_config()
        },
    );

    service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "hello".to_string(),
            correlation_id: None,
        })
        .await
        .expect("chat should succeed");

    let captured = llm_clone
        .first_message
        .lock()
        .unwrap()
        .take()
        .expect("LLM must have received at least one message");

    match captured {
        AgentMessage::System(prompt) => {
            assert_eq!(
                prompt, expected_prompt,
                "first message must be the configured system prompt"
            );
        }
        other => panic!("expected AgentMessage::System as first message, got: {other:?}"),
    }
}
