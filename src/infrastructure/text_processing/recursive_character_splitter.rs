use async_trait::async_trait;

use crate::application::ports::{TextSplitter, TextSplitterError};
use crate::domain::{Chunk, DocumentId};

pub struct RecursiveCharacterSplitter {
    chunk_size: usize,
    chunk_overlap: usize,
}

impl RecursiveCharacterSplitter {
    pub fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
        Self {
            chunk_size,
            chunk_overlap,
        }
    }
}

#[async_trait]
impl TextSplitter for RecursiveCharacterSplitter {
    async fn split(
        &self,
        text: &str,
        document_id: DocumentId,
    ) -> Result<Vec<Chunk>, TextSplitterError> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let total_len = chars.len();

        if total_len == 0 {
            return Ok(chunks);
        }

        let mut offset = 0;
        while offset < total_len {
            let end = (offset + self.chunk_size).min(total_len);
            let chunk_text: String = chars[offset..end].iter().collect();

            chunks.push(Chunk::new(chunk_text, document_id, None, offset));

            let step = if self.chunk_size > self.chunk_overlap {
                self.chunk_size - self.chunk_overlap
            } else {
                self.chunk_size
            };

            offset += step;
        }

        Ok(chunks)
    }
}
