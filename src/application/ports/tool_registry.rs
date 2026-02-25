use super::llm_client::ToolSchema;

pub trait ToolRegistry: Send + Sync {
    fn list_tools(&self) -> Vec<ToolSchema>;
}
