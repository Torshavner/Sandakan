//! @AI: application services routing map
//! - agent_service    -> AgentService: agentic ReAct loop (think → tool call → observe → answer).
//!   Exposes AgentServicePort trait (for dyn dispatch from AppState), AgentChatRequest/Response,
//!   AgentProgressEvent (Thinking | ToolCall | ToolResult), AgentError.
//!   When eval enabled, fires-and-forgets EvalEvent after each agent turn.
//! - eval_metrics    -> compute_faithfulness (LLM-as-judge scoring, extracts f32 from first line,
//!   rejects out-of-range values). Used by EvalWorker for background faithfulness scoring.
//! - eval_worker     -> EvalWorker: background polling worker. Claims pending outbox entries,
//!   runs faithfulness scoring via eval_metrics, persists EvalResult via EvalResultRepository,
//!   emits structured tracing events. Separates receive_batch() (transport, US-017 ready) from
//!   process_entry() (stable business logic). EvalWorkerError includes ResultRepository variant.
//! - ingestion_service -> IngestionService: synchronous document ingestion (load → split → embed → upsert).
//! - ingestion_worker  -> IngestionWorker: background actor consuming mpsc channel for async ingestion.
//! - retrieval_service -> RetrievalService: RAG query pipeline (embed → search → filter → generate).
//!   When eval enabled, fires-and-forgets EvalEvent + eval_outbox row after each query.
//! - token_counter     -> count_tokens(text): fast tiktoken-based token count for context trimming.

mod agent_service;
pub mod eval_metrics;
mod eval_worker;
mod ingestion_service;
mod ingestion_worker;
mod retrieval_service;
mod token_counter;

pub use agent_service::{
    AgentChatRequest, AgentChatResponse, AgentError, AgentProgressEvent, AgentService,
    AgentServicePort,
};
pub use eval_worker::{EvalWorker, EvalWorkerError};
pub use ingestion_service::{IngestionError, IngestionService};
pub use ingestion_worker::{IngestionMessage, IngestionWorker, IngestionWorkerError};
pub use retrieval_service::{QueryResponse, RetrievalError, RetrievalService, SourceChunk};
pub use token_counter::count_tokens;
