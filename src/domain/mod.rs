//! @AI: domain module routing map
//! - chunk          -> Chunk (text + metadata), ChunkId (UUID newtype), DocumentId (UUID newtype).
//!   Methods: new() (no metadata), with_metadata() (Arc<DocumentMetadata>), as_contextual_string().
//! - document_metadata -> DocumentMetadata (title, content_type, source_url). Arc Flyweight shared
//!   across all chunks from the same document. Constructor: from_document(&Document, Option<String>).
//! - conversation   -> Conversation aggregate with ordered Message history.
//! - conversation_id -> ConversationId (UUID newtype) with from_uuid / as_uuid.
//! - document       -> Document value object; ContentType enum (Pdf | Text | Audio).
//! - embedding      -> Embedding(Vec<f32>) with cosine_similarity(). No Default impl.
//! - eval_entry     -> EvalEntry ground-truth value object; serde Deserialize from JSONL.
//!   Fields: question (required), expected_answer (required), expected_source_pages: Option<Vec<u32>> (default None).
//! - eval_event     -> EvalEvent captured passively during RAG queries. EvalSource (text, page, score).
//!   EvalEventId with from_uuid / as_uuid. context_text() joins sources with "\n\n".
//!   EvalOperationType enum (Query | AgenticRun | IngestionPdf | IngestionMp4) with as_str().
//!   Constructors: new() → Query, new_agentic() → AgenticRun, new_ingestion() → Pdf/Mp4.
//! - eval_outbox    -> EvalOutboxEntry (durable outbox row for background scoring) and
//!   EvalOutboxStatus (Pending | Processing | Done | Failed). Serde-ready for US-017 broker bounds.
//! - eval_result    -> EvalResult persisted scoring outcome; EvalResultId (UUID newtype with from_uuid/as_uuid).
//!   Fields: faithfulness (f32), context_recall (Option<f32>), correctness (Option<f32>),
//!   below_threshold (bool, pre-computed at save time), computed_at (DateTime<Utc>).
//! - job            -> Job aggregate for ingestion pipeline lifecycle tracking.
//! - job_id         -> JobId (UUID newtype).
//! - job_status     -> JobStatus enum (Queued | Processing | Done | Failed).
//! - message        -> Message value object (id, conversation_id, role, content, tool_call_id, created_at).
//! - message_id     -> MessageId (UUID newtype).
//! - message_role   -> MessageRole enum (User | Assistant | System | Tool | ToolResponse) with as_str() / parse().
//! - storage_path   -> StoragePath value object for staging store file references.
//! - tool_call      -> ToolCallId (String newtype), ToolName (String newtype),
//!   ToolCall (id + name + arguments: Value), ToolResult (tool_call_id + tool_name + content).
//! - transcript_segment -> TranscriptSegment (text, start_time, end_time in seconds).
//!   merge_text() joins segments into a plain string.

mod chunk;
mod conversation;
mod conversation_id;
mod document;
mod document_metadata;
mod embedding;
mod eval_entry;
mod eval_event;
mod eval_outbox;
mod eval_result;
mod job;
mod job_id;
mod job_status;
mod message;
mod message_id;
mod message_role;
mod storage_path;
mod tool_call;
mod transcript_segment;

pub use chunk::{Chunk, ChunkId, DocumentId};
pub use conversation::Conversation;
pub use conversation_id::ConversationId;
pub use document::{ContentType, Document};
pub use document_metadata::DocumentMetadata;
pub use embedding::Embedding;
pub use eval_entry::EvalEntry;
pub use eval_event::{EvalEvent, EvalEventId, EvalOperationType, EvalSource};
pub use eval_outbox::{EvalOutboxEntry, EvalOutboxStatus};
pub use eval_result::{EvalResult, EvalResultId};
pub use job::Job;
pub use job_id::JobId;
pub use job_status::JobStatus;
pub use message::Message;
pub use message_id::MessageId;
pub use message_role::MessageRole;
pub use storage_path::StoragePath;
pub use tool_call::{ToolCall, ToolCallId, ToolName, ToolResult};
pub use transcript_segment::TranscriptSegment;
