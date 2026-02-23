//! @AI: persistence module routing map
//! - eval_event     -> EvalEventRepository adapters: JsonlEvalEventRepository (append-only JSONL,
//!   useful for offline CLI runs without a DB connection).
//! - pg_pool        -> create_pool(): creates a PgPool from DATABASE_URL with max_connections cap.
//! - repositories   -> PostgreSQL and mock adapters for Conversation, Job, EvalEvent, EvalOutbox,
//!   and EvalResult ports. PgEvalResultRepository stores scored results; UNIQUE(eval_event_id)
//!   enforces one result per event. MockEvalResultRepository for unit tests.
//! - vector_store   -> QdrantAdapter implementing VectorStore; MockVectorStore for tests.

mod eval_event;
mod pg_pool;
mod repositories;
mod vector_store;

pub use eval_event::JsonlEvalEventRepository;

pub use repositories::MockConversationRepository;
pub use repositories::MockEvalEventRepository;
pub use repositories::MockEvalOutboxRepository;
pub use repositories::MockEvalResultRepository;
pub use repositories::MockJobRepository;
pub use repositories::PgConversationRepository;
pub use repositories::PgEvalEventRepository;
pub use repositories::PgEvalOutboxRepository;
pub use repositories::PgEvalResultRepository;
pub use repositories::PgJobRepository;

pub use pg_pool::create_pool;

pub use vector_store::MockVectorStore;
pub use vector_store::MockVectorStoreLowScore;
pub use vector_store::QdrantAdapter;
