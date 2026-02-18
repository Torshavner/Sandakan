use crate::application::ports::{LlmClient, LlmClientError};

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
    ) -> Result<
        std::pin::Pin<
            Box<
                dyn futures::stream::Stream<Item = Result<String, LlmClientError>> + Send + 'static,
            >,
        >,
        LlmClientError,
    > {
        Ok(Box::pin(futures::stream::once(async {
            Ok("Mock answer".to_string())
        })))
    }
}