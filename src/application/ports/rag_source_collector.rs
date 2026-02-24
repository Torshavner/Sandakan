use crate::domain::EvalSource;

/// Side-channel accumulator that lets `RagSearchAdapter` (writer) share retrieved
/// chunks with `AgentService` (reader) without coupling the two directly.
///
/// Both methods are sync; no async executor required.
pub trait RagSourceCollector: Send + Sync {
    /// Append `sources` to the accumulator.
    fn collect(&self, sources: Vec<EvalSource>);

    /// Drain and return all accumulated sources, leaving the accumulator empty.
    fn drain(&self) -> Vec<EvalSource>;
}
