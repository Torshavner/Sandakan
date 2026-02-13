use async_trait::async_trait;
use futures::stream::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::application::ports::{LlmClient, LlmClientError, LlmTokenStream};
use crate::presentation::config::LlmSettings;

pub struct StreamingLlmClient {
    client: Client,
    provider: String,
    base_url: String,
    api_key: String,
    model: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone)]
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
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
}

impl StreamingLlmClient {
    fn build_messages(&self, prompt: &str, context: &str) -> Vec<ChatMessage> {
        let system_content = self.system_prompt_template.replace("{context}", context);
        vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_content,
            },
            ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            },
        ]
    }

    fn apply_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.provider == "azure" {
            request.header("api-key", &self.api_key)
        } else {
            request.header("Authorization", format!("Bearer {}", self.api_key))
        }
    }
}

#[async_trait]
impl LlmClient for StreamingLlmClient {
    async fn complete(&self, prompt: &str, context: &str) -> Result<String, LlmClientError> {
        let messages = self.build_messages(prompt, context);
        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            stream: None,
        };

        let request = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .json(&request_body);
        let response = self
            .apply_auth(request)
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

    async fn complete_stream(
        &self,
        prompt: &str,
        context: &str,
    ) -> Result<LlmTokenStream, LlmClientError> {
        let messages = self.build_messages(prompt, context);
        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            stream: Some(true),
        };

        let request = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .json(&request_body);
        let response = self
            .apply_auth(request)
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

        let stream = response.bytes_stream();
        let token_stream = Box::pin(stream.flat_map(|chunk_result| {
            let items: Vec<Result<String, LlmClientError>> = match chunk_result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let mut tokens = Vec::new();
                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                break;
                            }
                            if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                                if let Some(choice) = chunk.choices.first() {
                                    if let Some(content) = &choice.delta.content {
                                        tokens.push(Ok(content.clone()));
                                    }
                                }
                            }
                        }
                    }
                    tokens
                }
                Err(e) => vec![Err(LlmClientError::ApiRequestFailed(e.to_string()))],
            };
            futures::stream::iter(items)
        }));

        Ok(token_stream)
    }
}

pub fn create_streaming_llm_client(
    settings: &LlmSettings,
    system_prompt_template: String,
) -> Result<StreamingLlmClient, LlmClientError> {
    let base_url = match settings.provider.as_str() {
        "openai" => "https://api.openai.com/v1".to_string(),
        "lmstudio" => settings
            .base_url
            .clone()
            .ok_or_else(|| {
                LlmClientError::InvalidResponse(
                    "base_url required for lmstudio provider".to_string(),
                )
            })?
            .trim_end_matches('/')
            .to_string(),
        "azure" => {
            let endpoint = settings.azure_endpoint.as_ref().ok_or_else(|| {
                LlmClientError::InvalidResponse(
                    "azure_endpoint required for azure provider".to_string(),
                )
            })?;
            format!(
                "{}/openai/deployments/{}",
                endpoint.trim_end_matches('/'),
                settings.chat_model
            )
        }
        _ => {
            return Err(LlmClientError::InvalidResponse(format!(
                "unknown provider: {}",
                settings.provider
            )));
        }
    };

    Ok(StreamingLlmClient {
        client: Client::new(),
        provider: settings.provider.clone(),
        base_url,
        api_key: settings.api_key.clone(),
        model: settings.chat_model.clone(),
        max_tokens: settings.max_tokens,
        temperature: settings.temperature,
        system_prompt_template,
    })
}
