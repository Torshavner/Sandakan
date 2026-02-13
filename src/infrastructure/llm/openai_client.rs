use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::application::ports::{LlmClient, LlmClientError};

pub struct OpenAiClient {
    client: Client,
    api_key: String,
    completion_model: String,
    max_tokens: usize,
    temperature: f32,
    system_prompt_template: String,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: usize,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: String,
}

impl OpenAiClient {
    pub fn new(
        api_key: String,
        completion_model: String,
        max_tokens: usize,
        temperature: f32,
        system_prompt_template: String,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key,
            completion_model,
            max_tokens,
            temperature,
            system_prompt_template,
        }
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn complete_stream(
        &self,
        _prompt: &str,
        _context: &str,
    ) -> Result<crate::application::ports::LlmTokenStream, LlmClientError> {
        Err(LlmClientError::InvalidResponse(
            "streaming not supported in legacy OpenAiClient".to_string(),
        ))
    }

    async fn complete(&self, prompt: &str, context: &str) -> Result<String, LlmClientError> {
        let system_message_content = self.system_prompt_template.replace("{context}", context);

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_message_content,
            },
            ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            },
        ];

        let request_body = ChatCompletionRequest {
            model: self.completion_model.clone(),
            messages,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| LlmClientError::ApiRequestFailed(e.to_string()))?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(LlmClientError::RateLimited);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(LlmClientError::ApiRequestFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let completion_response: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| LlmClientError::InvalidResponse(e.to_string()))?;

        completion_response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or_else(|| LlmClientError::InvalidResponse("empty choices".to_string()))
    }
}
