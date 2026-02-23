//! @AI: repositories module routing map
//! - mock_repository            -> In-memory stubs: MockConversationRepository, MockJobRepository,
//!   MockEvalEventRepository, MockEvalOutboxRepository, MockEvalResultRepository.
//!   Used in offline unit/integration tests only.
//! - pg_conversation_repository -> PostgreSQL adapter for ConversationRepository port.
//!   Persists conversations and messages; get_messages returns oldest-first.
//! - pg_eval_event_repository   -> PostgreSQL adapter for EvalEventRepository port.
//!   Stores eval events as rows with JSONB retrieved_sources. sample() uses ORDER BY RANDOM().
//! - pg_eval_outbox_repository  -> PostgreSQL adapter for EvalOutboxRepository port.
//!   Outbox pattern: enqueue pending rows, claim_pending with FOR UPDATE SKIP LOCKED,
//!   mark_done/mark_failed. Partial index on status='pending' for fast polling.
//! - pg_eval_result_repository  -> PostgreSQL adapter for EvalResultRepository port.
//!   INSERT with ON CONFLICT (eval_event_id) DO NOTHING for idempotent worker retry safety.
//!   UNIQUE(eval_event_id) enforces one result per event.
//! - pg_job_repository          -> PostgreSQL adapter for JobRepository port.
//!   Tracks ingestion job lifecycle (QUEUED → PROCESSING → DONE/FAILED).

mod mock_repository;
mod pg_conversation_repository;
mod pg_eval_event_repository;
mod pg_eval_outbox_repository;
mod pg_eval_result_repository;
mod pg_job_repository;

pub use mock_repository::MockConversationRepository;
pub use mock_repository::MockEvalEventRepository;
pub use mock_repository::MockEvalOutboxRepository;
pub use mock_repository::MockEvalResultRepository;
pub use mock_repository::MockJobRepository;
pub use pg_conversation_repository::PgConversationRepository;
pub use pg_eval_event_repository::PgEvalEventRepository;
pub use pg_eval_outbox_repository::PgEvalOutboxRepository;
pub use pg_eval_result_repository::PgEvalResultRepository;
pub use pg_job_repository::PgJobRepository;
