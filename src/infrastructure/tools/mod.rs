//! @AI: infrastructure/tools — concrete tool adapter routing map
//! - in_memory_rag_source_collector -> InMemoryRagSourceCollector: implements RagSourceCollector
//!   using Arc<Mutex<Vec<EvalSource>>>. Shared between RagSearchAdapter (writer) and AgentService
//!   (reader). Only created when agent.rag_search_enabled = true AND eval.enabled = true.
//! - rag_search_adapter       -> RagSearchAdapter: implements ToolHandler for kb vector search.
//!   Calls RetrievalServicePort::search_chunks (embed + search + filter, no LLM synthesis).
//!   Formats raw SourceChunks as numbered list truncated to 800 chars each.
//!   Optionally holds Arc<dyn RagSourceCollector> to populate eval sources side-channel.
//! - web_search_adapter       -> WebSearchAdapter: implements ToolHandler for Brave Search API.
//!   Exposes tool_schema() → ToolSchema for registration with StaticToolRegistry.
//!   Auth: X-Subscription-Token header. Response: formats top-N results as numbered list.
//! - notification_adapter     -> NotificationAdapter: implements ToolHandler for HTTP webhook delivery.
//!   Supports Plain and Slack body formats. Registered only when agent.notification.webhook_url is set.
//!   build_body() is a pure fn — testable without HTTP.
//! - static_tool_registry     -> StaticToolRegistry: implements ToolRegistry with a fixed Vec<ToolSchema>.

mod in_memory_rag_source_collector;
mod notification_adapter;
mod rag_search_adapter;
mod static_tool_registry;
mod web_search_adapter;

pub use in_memory_rag_source_collector::InMemoryRagSourceCollector;
pub use notification_adapter::{
    NotificationAdapter, NotificationConfig, NotificationFormat,
    build_body as build_notification_body,
};
pub use rag_search_adapter::RagSearchAdapter;
pub use static_tool_registry::StaticToolRegistry;
pub use web_search_adapter::{WebSearchAdapter, WebSearchConfig};
