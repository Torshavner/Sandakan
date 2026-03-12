use async_trait::async_trait;
use reqwest::Client;

use crate::application::ports::{McpError, ToolSchema};
use crate::infrastructure::mcp::ToolHandler;

pub struct LinkedInConfig {
    pub access_token: String,
    /// LinkedIn URN identifying the author, e.g. `"urn:li:person:xxxx"`.
    pub author_urn: String,
    pub timeout_secs: u64,
}

pub struct LinkedInAdapter {
    client: Client,
    config: LinkedInConfig,
}

impl LinkedInAdapter {
    pub fn new(config: LinkedInConfig) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()?;
        Ok(Self { client, config })
    }

    pub fn tool_schema() -> ToolSchema {
        ToolSchema {
            name: "post_linkedin".to_string(),
            description: "Publish a post on LinkedIn on behalf of the configured author. \
                Call this when the user asks to share, announce, or post something to LinkedIn. \
                The content MUST be professional, authentic, and concise (5–7 sentences). \
                It MUST describe what the system does and how it was built, \
                state that it was created as part of the Ciklum AI Academy, \
                and optionally mention @Ciklum."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The text body of the LinkedIn post. Must be 5–7 sentences, professional and authentic. Must explain what the system does, how it was built, and mention the Ciklum AI Academy. Optionally tag @Ciklum."
                    },
                    "visibility": {
                        "type": "string",
                        "enum": ["PUBLIC", "CONNECTIONS"],
                        "description": "Audience for the post. Defaults to PUBLIC."
                    }
                },
                "required": ["content"]
            }),
        }
    }
}

/// Builds the LinkedIn UGC Posts API request body.
///
/// Extracted as a pure function so tests can validate the payload shape
/// without making real HTTP calls.
pub fn build_ugc_post(author_urn: &str, content: &str, visibility: &str) -> serde_json::Value {
    serde_json::json!({
        "author": author_urn,
        "lifecycleState": "PUBLISHED",
        "specificContent": {
            "com.linkedin.ugc.ShareContent": {
                "shareCommentary": {
                    "text": content
                },
                "shareMediaCategory": "NONE"
            }
        },
        "visibility": {
            "com.linkedin.ugc.MemberNetworkVisibility": visibility
        }
    })
}

#[async_trait]
impl ToolHandler for LinkedInAdapter {
    fn tool_name(&self) -> &str {
        "post_linkedin"
    }

    async fn execute(&self, arguments: &serde_json::Value) -> Result<String, McpError> {
        let content = arguments["content"]
            .as_str()
            .ok_or_else(|| McpError::Serialization("missing 'content' argument".to_string()))?;

        let visibility = arguments["visibility"].as_str().unwrap_or("PUBLIC");

        let body = build_ugc_post(&self.config.author_urn, content, visibility);

        let response = self
            .client
            .post("https://api.linkedin.com/v2/ugcPosts")
            .header(
                "Authorization",
                format!("Bearer {}", self.config.access_token),
            )
            .header("Content-Type", "application/json")
            .header("X-Restli-Protocol-Version", "2.0.0")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    McpError::ExecutionFailed("LinkedIn API timeout".to_string())
                } else {
                    McpError::ExecutionFailed(e.to_string())
                }
            })?;

        if response.status().as_u16() == 201 {
            return Ok("Post published successfully.".to_string());
        }

        Err(McpError::ExecutionFailed(format!(
            "LinkedIn API HTTP {}",
            response.status().as_u16()
        )))
    }
}

// ─── Mimic adapter ────────────────────────────────────────────────────────────

/// A dry-run variant of the LinkedIn post tool.
///
/// Instead of calling the LinkedIn API, it formats the would-be post as a
/// `[linkedin_preview]` result string. The agent's existing `ToolResult` SSE
/// event carries this to the client, so the user sees the post content without
/// any real API call being made.
pub struct LinkedInMimicAdapter;

#[async_trait]
impl ToolHandler for LinkedInMimicAdapter {
    fn tool_name(&self) -> &str {
        "post_linkedin"
    }

    async fn execute(&self, arguments: &serde_json::Value) -> Result<String, McpError> {
        let content = arguments["content"]
            .as_str()
            .ok_or_else(|| McpError::Serialization("missing 'content' argument".to_string()))?;

        let visibility = arguments["visibility"].as_str().unwrap_or("PUBLIC");

        Ok(format!(
            "[linkedin_preview] visibility={visibility}\n\n{content}"
        ))
    }
}
