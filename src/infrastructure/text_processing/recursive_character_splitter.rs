use std::sync::Arc;

use async_trait::async_trait;

use crate::application::ports::{TextSplitter, TextSplitterError};
use crate::domain::{Chunk, DocumentId, DocumentMetadata, TranscriptSegment};

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
        metadata: Option<Arc<DocumentMetadata>>,
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

            let chunk = match &metadata {
                Some(meta) => {
                    Chunk::with_metadata(chunk_text, document_id, None, offset, Arc::clone(meta))
                }
                None => Chunk::new(chunk_text, document_id, None, offset),
            };
            chunks.push(chunk);

            let step = if self.chunk_size > self.chunk_overlap {
                self.chunk_size - self.chunk_overlap
            } else {
                self.chunk_size
            };

            offset += step;
        }

        Ok(chunks)
    }

    /// Merges segment text and splits it using the character-based chunker.
    /// `start_time` is assigned to each chunk proportionally from the merged text offset.
    async fn split_segments(
        &self,
        segments: &[TranscriptSegment],
        document_id: DocumentId,
        metadata: Option<Arc<DocumentMetadata>>,
    ) -> Result<Vec<Chunk>, TextSplitterError> {
        let merged = TranscriptSegment::merge_text(segments);
        let chunks = self.split(&merged, document_id, metadata).await?;

        // Attach the start_time of the first segment to every chunk (character-level
        // splitter does not track segment boundaries, so this is a best-effort approximation).
        let first_start = segments.first().map(|s| s.start_time);
        Ok(chunks
            .into_iter()
            .map(|c| match first_start {
                Some(t) => c.with_start_time(t),
                None => c,
            })
            .collect())
    }
}
