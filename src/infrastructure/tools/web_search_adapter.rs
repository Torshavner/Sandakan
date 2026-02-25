use async_trait::async_trait;
use reqwest::Client;

use crate::application::ports::{McpError, ToolSchema};
use crate::infrastructure::mcp::ToolHandler;

pub struct WebSearchConfig {
    pub api_key: String,
    /// Brave Search API endpoint.
    /// Default: `https://api.search.brave.com/res/v1/web/search`
    pub endpoint: String,
    pub max_results: usize,
}

pub struct WebSearchAdapter {
    client: Client,
    config: WebSearchConfig,
}

impl WebSearchAdapter {
    pub fn new(config: WebSearchConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// JSON Schema for this tool, registered with the `ToolRegistry`.
    pub fn tool_schema() -> ToolSchema {
        ToolSchema {
            name: "web_search".to_string(),
            description: "Search the web for up-to-date information using a query string."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to execute"
                    }
                },
                "required": ["query"]
            }),
        }
    }
}

#[async_trait]
impl ToolHandler for WebSearchAdapter {
    fn tool_name(&self) -> &str {
        "web_search"
    }

    async fn execute(&self, arguments: &serde_json::Value) -> Result<String, McpError> {
        let query = arguments["query"]
            .as_str()
            .ok_or_else(|| McpError::Serialization("missing 'query' argument".to_string()))?;

        let response = self
            .client
            .get(&self.config.endpoint)
            .header("Accept", "application/json")
            .header("X-Subscription-Token", &self.config.api_key)
            .query(&[
                ("q", query),
                ("count", &self.config.max_results.to_string()),
            ])
            .send()
            .await
            .map_err(|e| McpError::ExecutionFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(McpError::ExecutionFailed(format!(
                "Brave Search HTTP {}",
                response.status()
            )));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| McpError::Serialization(e.to_string()))?;

        Ok(format_brave_results(&body, self.config.max_results))
    }
}

fn format_brave_results(body: &serde_json::Value, limit: usize) -> String {
    let results = body["web"]["results"].as_array();
    match results {
        None => "No results found.".to_string(),
        Some(items) if items.is_empty() => "No results found.".to_string(),
        Some(items) => items
            .iter()
            .take(limit)
            .enumerate()
            .map(|(i, r)| {
                format!(
                    "{}. {}\n{}\n{}",
                    i + 1,
                    r["title"].as_str().unwrap_or("(no title)"),
                    r["url"].as_str().unwrap_or(""),
                    r["description"].as_str().unwrap_or(""),
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}
