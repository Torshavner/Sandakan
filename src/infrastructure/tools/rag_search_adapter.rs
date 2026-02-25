use std::sync::Arc;

use async_trait::async_trait;

use crate::application::ports::{
    McpError, RagSourceCollector, RetrievalServicePort, SourceChunk, ToolSchema,
};
use crate::domain::EvalSource;
use crate::infrastructure::mcp::ToolHandler;

pub struct RagSearchAdapter {
    port: Arc<dyn RetrievalServicePort>,
    source_collector: Option<Arc<dyn RagSourceCollector>>,
}

impl RagSearchAdapter {
    pub fn new(
        port: Arc<dyn RetrievalServicePort>,
        source_collector: Option<Arc<dyn RagSourceCollector>>,
    ) -> Self {
        Self {
            port,
            source_collector,
        }
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

        if let Some(collector) = &self.source_collector {
            let eval_sources: Vec<EvalSource> = chunks
                .iter()
                .map(|c| EvalSource {
                    text: c.text.clone(),
                    page: c.page,
                    score: c.score,
                })
                .collect();
            collector.collect(eval_sources);
        }

        Ok(format_rag_response(&chunks))
    }
}

/// Returns the longest prefix of `s` whose byte length does not exceed `max_bytes`,
/// always slicing at a UTF-8 codepoint boundary to avoid a panic on multi-byte sequences.
fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let boundary = s
        .char_indices()
        .take_while(|(pos, ch)| pos + ch.len_utf8() <= max_bytes)
        .last()
        .map(|(pos, ch)| pos + ch.len_utf8())
        .unwrap_or(0);
    &s[..boundary]
}

fn format_rag_response(chunks: &[SourceChunk]) -> String {
    if chunks.is_empty() {
        return "No relevant documents found in the knowledge base.".to_string();
    }

    let entries: Vec<String> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let location_label = match (chunk.page, chunk.start_time) {
                (_, Some(t)) => format!("{:.1}s", t),
                (Some(p), None) => format!("Page {p}"),
                (None, None) => "Page ?".to_string(),
            };

            let label = match &chunk.title {
                Some(title) => format!("{title} - {location_label}"),
                None => location_label,
            };

            let text = truncate_utf8(&chunk.text, 800);

            format!(
                "{}. [{}, score: {:.2}]: {}",
                i + 1,
                label,
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
