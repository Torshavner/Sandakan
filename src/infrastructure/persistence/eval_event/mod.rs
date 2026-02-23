//! @AI: eval_event persistence module routing map
//! - jsonl_eval_event_repository -> Append-only JSONL file implementation of EvalEventRepository.
//!   record() creates parent dirs and appends one JSON line. list(limit) reads all lines,
//!   sample(n) uses Fisher-Yates partial shuffle with a time-seeded rng. Returns empty vec
//!   (not error) when the file does not yet exist. Useful for offline/CLI evaluation runs.

mod jsonl_eval_event_repository;

pub use jsonl_eval_event_repository::JsonlEvalEventRepository;
