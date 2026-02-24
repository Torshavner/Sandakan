use std::sync::Arc;

use async_trait::async_trait;

use crate::application::ports::{McpError, RetrievalServicePort, SourceChunk, ToolSchema};
use crate::infrastructure::mcp::ToolHandler;

pub struct RagSearchAdapter {
    port: Arc<dyn RetrievalServicePort>,
}

impl RagSearchAdapter {
    pub fn new(port: Arc<dyn RetrievalServicePort>) -> Self {
        Self { port }
    }

    /// JSON Schema for this tool, registered with the `ToolRegistry`.
    pub fn tool_schema() -> ToolSchema {
        ToolSchema {
            name: "rag_search".to_string(),
            description: "Search the uploaded knowledge base documents for relevant information. \
                Returns raw source passages. Call multiple times to refine results."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to execute against the knowledge base"
                    }
                },
                "required": ["query"]
            }),
        }
    }
}

#[async_trait]
impl ToolHandler for RagSearchAdapter {
    fn tool_name(&self) -> &str {
        "rag_search"
    }

    async fn execute(&self, arguments: &serde_json::Value) -> Result<String, McpError> {
        let query = arguments["query"]
            .as_str()
            .ok_or_else(|| McpError::Serialization("missing 'query' argument".to_string()))?;

        let chunks = self
            .port
            .search_chunks(query)
            .await
            .map_err(|e| McpError::ExecutionFailed(e.to_string()))?;

        Ok(format_rag_response(&chunks))
    }
}

fn format_rag_response(chunks: &[SourceChunk]) -> String {
    if chunks.is_empty() {
        return "No relevant documents found in the knowledge base.".to_string();
    }

    let entries: Vec<String> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let page_label = chunk
                .page
                .map(|p| format!("Page {p}"))
                .unwrap_or_else(|| "Page ?".to_string());

            let text = if chunk.text.len() > 800 {
                &chunk.text[..800]
            } else {
                &chunk.text
            };

            format!(
                "{}. [{}, score: {:.2}]: {}",
                i + 1,
                page_label,
                chunk.score,
                text
            )
        })
        .collect();

    format!(
        "Found {} relevant sources:\n{}",
        chunks.len(),
        entries.join("\n")
    )
}
