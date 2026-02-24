//! @AI: application ports (traits) routing map
//! - agent_message              -> AgentMessage enum (System | User | Assistant | ToolResult) used
//!   within the ReAct loop. Not persisted. Implements From<Message> for history hydration.
//! - collection_config          -> CollectionConfig value object for VectorStore creation.
//! - conversation_repository    -> ConversationRepository port: create, get, append_message, get_messages.
//! - distance_metric            -> DistanceMetric enum (Cosine | Dot | Euclidean).
//! - embedder                   -> Embedder port: embed(&str) and embed_batch(&[&str]).
//! - eval_event_repository      -> EvalEventRepository port: record, list(limit), sample(n).
//!   EvalEventError covers Io and Serialization failures.
//! - eval_outbox_repository     -> EvalOutboxRepository port: enqueue, claim_pending (FOR UPDATE
//!   SKIP LOCKED), mark_done, mark_failed. EvalOutboxError covers Database and Serialization.
//! - eval_result_repository     -> EvalResultRepository port: save(result). EvalResultError covers
//!   Database and Serialization. Used by EvalWorker to persist scored results.
//! - file_loader                -> FileLoader port: load(&[u8]) → String (PDF/text extraction).
//! - job_repository             -> JobRepository port: create, get, update_status, list_by_status.
//! - llm_client                 -> LlmClient port: complete(prompt, context), complete_stream,
//!   and complete_with_tools(messages, tools) → LlmToolResponse.
//!   Also exports ToolSchema and LlmToolResponse.
//! - mcp_client_port            -> McpClientPort port: call_tool(ToolCall) → ToolResult.
//!   McpError covers ToolNotFound, ExecutionFailed, Serialization, Transport, Protocol, ServerExited.
//! - payload_field_type         -> PayloadFieldType enum for Qdrant index configuration.
//! - payload_index              -> PayloadIndex struct for Qdrant payload indexing.
//! - repository_error           -> RepositoryError for conversation/job persistence failures.
//! - search_result              -> SearchResult: (Chunk, score) pair returned from VectorStore.
//! - staging_store              -> StagingStore port for temporary file upload storage.
//! - text_splitter              -> TextSplitter port: split(text) → Vec<String>.
//! - tool_registry              -> ToolRegistry port: list_tools() → Vec<ToolSchema> (sync).
//! - transcription_engine       -> TranscriptionEngine + AudioDecoder ports for audio ingestion.
//! - vector_store               -> VectorStore port: upsert, search, delete, collection management.
//! - vector_store_error         -> VectorStoreError for vector store operation failures.

mod agent_message;
mod collection_config;
mod conversation_repository;
mod distance_metric;
mod embedder;
mod eval_event_repository;
mod eval_outbox_repository;
mod eval_result_repository;
mod file_loader;
mod job_repository;
mod llm_client;
mod mcp_client_port;
mod payload_field_type;
mod payload_index;
mod repository_error;
mod search_result;
mod staging_store;
mod text_splitter;
mod tool_registry;
mod transcription_engine;
mod vector_store;
mod vector_store_error;

pub use agent_message::AgentMessage;
pub use collection_config::CollectionConfig;
pub use conversation_repository::ConversationRepository;
pub use distance_metric::DistanceMetric;
pub use embedder::{Embedder, EmbedderError};
pub use eval_event_repository::{EvalEventError, EvalEventRepository};
pub use eval_outbox_repository::{EvalOutboxError, EvalOutboxRepository};
pub use eval_result_repository::{EvalResultError, EvalResultRepository};
pub use file_loader::{FileLoader, FileLoaderError};
pub use job_repository::JobRepository;
pub use llm_client::{LlmClient, LlmClientError, LlmTokenStream, LlmToolResponse, ToolSchema};
pub use mcp_client_port::{McpClientPort, McpError};
pub use payload_field_type::PayloadFieldType;
pub use payload_index::PayloadIndex;
pub use repository_error::RepositoryError;
pub use search_result::SearchResult;
pub use staging_store::{StagingStore, StagingStoreError};
pub use text_splitter::{TextSplitter, TextSplitterError};
pub use tool_registry::ToolRegistry;
pub use transcription_engine::{
    AudioDecoder, AudioDecoderError, TranscriptionEngine, TranscriptionError,
};
pub use vector_store::VectorStore;
pub use vector_store_error::VectorStoreError;
