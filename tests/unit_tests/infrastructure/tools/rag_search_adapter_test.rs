use async_trait::async_trait;
use sandakan::application::ports::{McpError, RetrievalError, RetrievalServicePort, SourceChunk};
use sandakan::infrastructure::mcp::ToolHandler;
use sandakan::infrastructure::tools::RagSearchAdapter;
use serde_json::json;
use std::sync::Arc;

// ─── Hand-written stubs ──────────────────────────────────────────────────────

struct StubPortWithChunks {
    chunks: Vec<SourceChunk>,
}

#[async_trait]
impl RetrievalServicePort for StubPortWithChunks {
    async fn search_chunks(&self, _query: &str) -> Result<Vec<SourceChunk>, RetrievalError> {
        Ok(self.chunks.clone())
    }
}

struct StubPortEmpty;

#[async_trait]
impl RetrievalServicePort for StubPortEmpty {
    async fn search_chunks(&self, _query: &str) -> Result<Vec<SourceChunk>, RetrievalError> {
        Ok(Vec::new())
    }
}

struct StubPortFailing;

#[async_trait]
impl RetrievalServicePort for StubPortFailing {
    async fn search_chunks(&self, _query: &str) -> Result<Vec<SourceChunk>, RetrievalError> {
        Err(RetrievalError::Embedding(
            sandakan::application::ports::EmbedderError::ApiRequestFailed(
                "vector store down".to_string(),
            ),
        ))
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn given_chunks_returned_by_port_when_executing_rag_search_then_formats_numbered_list() {
    let chunks = vec![
        SourceChunk {
            text: "Rust is a systems programming language.".to_string(),
            page: Some(1),
            score: 0.95,
            title: None,
            source_url: None,
            content_type: None,
            start_time: None,
        },
        SourceChunk {
            text: "It focuses on safety and performance.".to_string(),
            page: Some(2),
            score: 0.88,
            title: None,
            source_url: None,
            content_type: None,
            start_time: None,
        },
    ];
    let adapter = RagSearchAdapter::new(Arc::new(StubPortWithChunks { chunks }), None);

    let result = adapter.execute(&json!({"query": "what is Rust?"})).await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("Found 2 relevant sources:"));
    assert!(output.contains("1. [Page 1, score: 0.95]:"));
    assert!(output.contains("2. [Page 2, score: 0.88]:"));
}

#[tokio::test]
async fn given_empty_knowledge_base_when_executing_rag_search_then_returns_not_found_message() {
    let adapter = RagSearchAdapter::new(Arc::new(StubPortEmpty), None);

    let result = adapter.execute(&json!({"query": "anything"})).await;

    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        "No relevant documents found in the knowledge base."
    );
}

#[tokio::test]
async fn given_missing_query_argument_when_executing_rag_search_then_returns_serialization_error() {
    let adapter = RagSearchAdapter::new(Arc::new(StubPortEmpty), None);

    let result = adapter.execute(&json!({})).await;

    assert!(matches!(result, Err(McpError::Serialization(_))));
    if let Err(McpError::Serialization(msg)) = result {
        assert!(msg.contains("missing 'query' argument"));
    }
}

#[tokio::test]
async fn given_port_failure_when_executing_rag_search_then_returns_execution_failed_error() {
    let adapter = RagSearchAdapter::new(Arc::new(StubPortFailing), None);

    let result = adapter.execute(&json!({"query": "trigger failure"})).await;

    assert!(matches!(result, Err(McpError::ExecutionFailed(_))));
}

#[tokio::test]
async fn given_chunk_text_exceeds_800_chars_when_formatting_response_then_text_is_truncated() {
    let long_text = "a".repeat(1000);
    let chunks = vec![SourceChunk {
        text: long_text,
        page: None,
        score: 0.80,
        title: None,
        source_url: None,
        content_type: None,
        start_time: None,
    }];
    let adapter = RagSearchAdapter::new(Arc::new(StubPortWithChunks { chunks }), None);

    let result = adapter.execute(&json!({"query": "long document"})).await;

    assert!(result.is_ok());
    let output = result.unwrap();
    // 800 'a' chars plus surrounding formatting — total output must NOT contain 1000 'a's
    assert!(!output.contains(&"a".repeat(801)));
}

#[tokio::test]
async fn given_rag_search_adapter_when_querying_tool_name_then_returns_rag_search() {
    let adapter = RagSearchAdapter::new(Arc::new(StubPortEmpty), None);
    assert_eq!(adapter.tool_name(), "rag_search");
}

#[tokio::test]
async fn given_rag_search_tool_schema_when_inspected_then_has_required_query_parameter() {
    let schema = RagSearchAdapter::tool_schema();
    assert_eq!(schema.name, "rag_search");
    assert!(schema.parameters["properties"]["query"].is_object());
    let required = schema.parameters["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v.as_str() == Some("query")));
}
