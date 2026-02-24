// @AI-BYPASS-LENGTH: all mocks and tests for AgentService live in one place to keep
// mock wiring transparent; splitting would obscure the test intent.
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use sandakan::application::ports::{
    AgentMessage, ConversationRepository, LlmClient, LlmClientError, LlmTokenStream,
    LlmToolResponse, McpClientPort, McpError, RepositoryError, ToolRegistry, ToolSchema,
};
use sandakan::application::services::{
    AgentChatRequest, AgentError, AgentService, AgentServicePort,
};
use sandakan::domain::{
    Conversation, ConversationId, Message, ToolCall, ToolCallId, ToolName, ToolResult,
};

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

fn build_service(llm: Arc<dyn LlmClient>, mcp: Arc<dyn McpClientPort>) -> AgentService {
    AgentService::new(
        llm,
        mcp,
        Arc::new(MockToolRegistry),
        Arc::new(MockConversationRepository),
        None,
        None,
        "test/model".to_string(),
        3,
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
async fn given_mcp_client_fails_when_executing_tool_then_agent_returns_tool_error() {
    let llm = Arc::new(MockLlmToolThenContent::new());
    let mcp = Arc::new(MockMcpFailing);
    let service = build_service(llm, mcp);

    let request = AgentChatRequest {
        conversation_id: None,
        user_message: "Search something".to_string(),
    };

    let result = service.chat(request).await;
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
async fn given_one_of_two_parallel_tool_calls_fails_when_executing_then_agent_returns_tool_error() {
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
    let service = build_service(llm, mcp);

    let result = service
        .chat(AgentChatRequest {
            conversation_id: None,
            user_message: "Do two things, one will fail".to_string(),
        })
        .await;

    assert!(
        matches!(result, Err(AgentError::Tool(_))),
        "expected Err(Tool(_))"
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
