use crate::application::ports::{ToolRegistry, ToolSchema};

/// A simple registry that holds a fixed list of tool schemas.
///
/// Constructed once at startup from the enabled tool adapters and wired into
/// `AgentService`. The registry is read-only after construction.
pub struct StaticToolRegistry {
    schemas: Vec<ToolSchema>,
}

impl StaticToolRegistry {
    pub fn new(schemas: Vec<ToolSchema>) -> Self {
        Self { schemas }
    }
}

#[async_trait::async_trait]
impl ToolRegistry for StaticToolRegistry {
    fn list_tools(&self) -> Vec<ToolSchema> {
        self.schemas.clone()
    }
}
