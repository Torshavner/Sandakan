use crate::application::ports::{
    AgentMessage, LlmClient, LlmClientError, LlmTokenStream, LlmToolResponse, ToolSchema,
};

pub struct MockLlmClient;

#[async_trait::async_trait]
impl LlmClient for MockLlmClient {
    async fn complete(&self, _prompt: &str, _context: &str) -> Result<String, LlmClientError> {
        Ok("Mock answer".to_string())
    }

    async fn complete_stream(
        &self,
        _prompt: &str,
        _context: &str,
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::once(async {
            Ok("Mock answer".to_string())
        })))
    }

    async fn complete_stream_with_messages(
        &self,
        _messages: &[AgentMessage],
    ) -> Result<LlmTokenStream, LlmClientError> {
        Ok(Box::pin(futures::stream::once(async {
            Ok("Mock agent answer".to_string())
        })))
    }

    async fn complete_with_tools(
        &self,
        _messages: &[AgentMessage],
        _tools: &[ToolSchema],
    ) -> Result<LlmToolResponse, LlmClientError> {
        Ok(LlmToolResponse::Content("Mock agent answer".to_string()))
    }
}
