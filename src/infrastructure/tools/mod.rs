//! @AI: infrastructure/tools — concrete tool adapter routing map
//! - web_search_adapter    -> WebSearchAdapter: implements ToolHandler for Brave Search API.
//!   Exposes tool_schema() → ToolSchema for registration with StaticToolRegistry.
//!   Auth: X-Subscription-Token header. Response: formats top-N results as numbered list.
//! - static_tool_registry  -> StaticToolRegistry: implements ToolRegistry with a fixed Vec<ToolSchema>.

mod static_tool_registry;
mod web_search_adapter;

pub use static_tool_registry::StaticToolRegistry;
pub use web_search_adapter::{WebSearchAdapter, WebSearchConfig};
