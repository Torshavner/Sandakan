//! @AI: infrastructure/tools — concrete tool adapter routing map
//! - rag_search_adapter    -> RagSearchAdapter: implements ToolHandler for kb vector search.
//!   Calls RetrievalServicePort::search_chunks (embed + search + filter, no LLM synthesis).
//!   Formats raw SourceChunks as numbered list truncated to 800 chars each.
//! - web_search_adapter    -> WebSearchAdapter: implements ToolHandler for Brave Search API.
//!   Exposes tool_schema() → ToolSchema for registration with StaticToolRegistry.
//!   Auth: X-Subscription-Token header. Response: formats top-N results as numbered list.
//! - static_tool_registry  -> StaticToolRegistry: implements ToolRegistry with a fixed Vec<ToolSchema>.

mod rag_search_adapter;
mod static_tool_registry;
mod web_search_adapter;

pub use rag_search_adapter::RagSearchAdapter;
pub use static_tool_registry::StaticToolRegistry;
pub use web_search_adapter::{WebSearchAdapter, WebSearchConfig};
