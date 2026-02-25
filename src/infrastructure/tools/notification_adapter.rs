use async_trait::async_trait;
use reqwest::Client;

use crate::application::ports::{McpError, ToolSchema};
use crate::infrastructure::mcp::ToolHandler;

pub enum NotificationFormat {
    Plain,
    Slack,
}

pub struct NotificationConfig {
    pub webhook_url: String,
    pub format: NotificationFormat,
    pub timeout_secs: u64,
}

pub struct NotificationAdapter {
    client: Client,
    config: NotificationConfig,
}

impl NotificationAdapter {
    pub fn new(config: NotificationConfig) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()?;
        Ok(Self { client, config })
    }

    /// JSON Schema for this tool, registered with the `ToolRegistry`.
    pub fn tool_schema() -> ToolSchema {
        ToolSchema {
            name: "send_notification".to_string(),
            description: "Send a notification to the configured webhook (Slack, Teams, or custom HTTP endpoint). \
                Call this when you have a final answer to push proactively to the user."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "The notification body text to deliver"
                    },
                    "title": {
                        "type": "string",
                        "description": "Optional subject or title for the notification"
                    }
                },
                "required": ["message"]
            }),
        }
    }
}

/// Builds the JSON payload sent to the webhook endpoint.
///
/// Extracted as a pure function so tests can validate body construction
/// without making real HTTP calls.
pub fn build_body(format: &NotificationFormat, title: &str, message: &str) -> serde_json::Value {
    match format {
        NotificationFormat::Plain => serde_json::json!({ "text": message }),
        NotificationFormat::Slack => {
            let text = if title.is_empty() {
                message.to_string()
            } else {
                format!("*{title}*\n{message}")
            };
            serde_json::json!({ "text": text })
        }
    }
}

#[async_trait]
impl ToolHandler for NotificationAdapter {
    fn tool_name(&self) -> &str {
        "send_notification"
    }

    async fn execute(&self, arguments: &serde_json::Value) -> Result<String, McpError> {
        let message = arguments["message"]
            .as_str()
            .ok_or_else(|| McpError::Serialization("missing 'message' argument".to_string()))?;

        let title = arguments["title"].as_str().unwrap_or("");

        let body = build_body(&self.config.format, title, message);

        let response = self
            .client
            .post(&self.config.webhook_url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    McpError::ExecutionFailed("webhook timeout".to_string())
                } else {
                    McpError::ExecutionFailed(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            return Err(McpError::ExecutionFailed(format!(
                "webhook HTTP {}",
                response.status().as_u16()
            )));
        }

        Ok("Notification sent successfully.".to_string())
    }
}
