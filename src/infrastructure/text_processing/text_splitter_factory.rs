use std::sync::Arc;

use crate::application::ports::TextSplitter;
use crate::presentation::config::ChunkingStrategy;

use super::{RecursiveCharacterSplitter, SemanticSplitter};

pub struct TextSplitterFactory;

impl TextSplitterFactory {
    pub fn create(
        strategy: ChunkingStrategy,
        max_chunk_size: usize,
        overlap: usize,
    ) -> Arc<dyn TextSplitter> {
        match strategy {
            ChunkingStrategy::Semantic => Arc::new(SemanticSplitter::new(max_chunk_size, overlap)),
            ChunkingStrategy::Fixed => {
                Arc::new(RecursiveCharacterSplitter::new(max_chunk_size, overlap))
            }
        }
    }
}
