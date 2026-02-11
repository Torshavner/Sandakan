use std::sync::Arc;

use crate::application::ports::TextSplitter;
use crate::presentation::config::EmbeddingStrategy;

use super::{RecursiveCharacterSplitter, SemanticSplitter};

pub struct TextSplitterFactory;

impl TextSplitterFactory {
    pub fn create(
        strategy: EmbeddingStrategy,
        max_chunk_size: usize,
        overlap: usize,
    ) -> Arc<dyn TextSplitter> {
        match strategy {
            EmbeddingStrategy::Semantic => Arc::new(SemanticSplitter::new(max_chunk_size, overlap)),
            EmbeddingStrategy::Fixed => {
                Arc::new(RecursiveCharacterSplitter::new(max_chunk_size, overlap))
            }
        }
    }
}
