use std::sync::{Arc, Mutex};

use crate::application::ports::RagSourceCollector;
use crate::domain::EvalSource;

/// Thread-safe in-memory accumulator for RAG sources collected during a single
/// agent ReAct loop. Shared (via `Arc`) between `RagSearchAdapter` (writer) and
/// `AgentService` (reader). `drain()` is called once after the loop completes.
pub struct InMemoryRagSourceCollector {
    sources: Arc<Mutex<Vec<EvalSource>>>,
}

impl InMemoryRagSourceCollector {
    pub fn new() -> Self {
        Self {
            sources: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for InMemoryRagSourceCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl RagSourceCollector for InMemoryRagSourceCollector {
    fn collect(&self, mut new_sources: Vec<EvalSource>) {
        // unwrap: mutex poisoning only occurs after a panic inside the critical
        // section, which is already a fatal unrecoverable state.
        self.sources.lock().unwrap().append(&mut new_sources);
    }

    fn drain(&self) -> Vec<EvalSource> {
        std::mem::take(&mut self.sources.lock().unwrap())
    }
}
