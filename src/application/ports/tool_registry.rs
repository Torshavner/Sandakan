use async_trait::async_trait;

use super::llm_client::ToolSchema;

#[async_trait]
pub trait ToolRegistry: Send + Sync {
    fn list_tools(&self) -> Vec<ToolSchema>;

    /// Returns the most relevant tools for a given intent.
    ///
    /// The default implementation returns all tools (capped at `top_k`),
    /// preserving backwards compatibility for static registries and mocks.
    async fn search_tools(&self, _intent: &str, top_k: usize) -> Vec<ToolSchema> {
        let all = self.list_tools();
        all.into_iter().take(top_k).collect()
    }
}
