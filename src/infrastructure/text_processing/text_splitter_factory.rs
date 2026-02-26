use std::sync::Arc;

use crate::application::ports::{TextSplitter, TextSplitterError};
use crate::presentation::config::ChunkingStrategy;

use super::{MarkdownSemanticSplitter, RecursiveCharacterSplitter, SemanticSplitter};

/// Paired splitters: `text` for plain text and audio/video, `markdown` for PDF.
pub struct TextSplitters {
    pub text: Arc<dyn TextSplitter>,
    pub markdown: Arc<dyn TextSplitter>,
}

pub struct TextSplitterFactory;

impl TextSplitterFactory {
    pub fn create(
        strategy: ChunkingStrategy,
        max_chunk_size: usize,
        overlap: usize,
    ) -> Result<TextSplitters, TextSplitterError> {
        match strategy {
            ChunkingStrategy::Semantic => Ok(TextSplitters {
                text: Arc::new(SemanticSplitter::new(max_chunk_size, overlap)?),
                markdown: Arc::new(MarkdownSemanticSplitter::new(max_chunk_size, overlap)?),
            }),
            ChunkingStrategy::Fixed => {
                let splitter: Arc<dyn TextSplitter> =
                    Arc::new(RecursiveCharacterSplitter::new(max_chunk_size, overlap));
                Ok(TextSplitters {
                    text: Arc::clone(&splitter),
                    markdown: splitter,
                })
            }
        }
    }
}
