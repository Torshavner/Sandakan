use std::sync::Arc;

use async_trait::async_trait;
use tiktoken_rs::{CoreBPE, cl100k_base};

use crate::application::ports::{TextSplitter, TextSplitterError};
use crate::domain::{Chunk, DocumentId, DocumentMetadata};

pub struct SemanticSplitter {
    max_tokens: usize,
    overlap_tokens: usize,
    // Initialized once at construction to avoid repeated BPE vocabulary parsing
    // in the inner loops of count_tokens.
    tokenizer: CoreBPE,
}

impl SemanticSplitter {
    pub fn new(max_tokens: usize, overlap_tokens: usize) -> Result<Self, TextSplitterError> {
        let tokenizer = cl100k_base().map_err(|e| {
            TextSplitterError::TokenizationFailed(format!("Failed to load tokenizer: {}", e))
        })?;
        Ok(Self {
            max_tokens,
            overlap_tokens,
            tokenizer,
        })
    }

    fn count_tokens(&self, text: &str) -> usize {
        self.tokenizer.encode_with_special_tokens(text).len()
    }

    fn split_into_sentences<'a>(&self, text: &'a str) -> Vec<&'a str> {
        let mut sentences: Vec<&'a str> = Vec::new();
        let mut start = 0;
        let mut chars = text.char_indices().peekable();

        while let Some((i, ch)) = chars.next() {
            if ch == '.' || ch == '!' || ch == '?' {
                // Sentence boundary only when followed by whitespace or end of text.
                let is_boundary = match chars.peek() {
                    None => true,
                    Some((_, next)) => next.is_whitespace(),
                };

                if is_boundary {
                    let slice = text[start..i + ch.len_utf8()].trim();
                    if !slice.is_empty() {
                        sentences.push(slice);
                    }
                    // Advance past the whitespace separator (not consumed yet).
                    start = i + ch.len_utf8();
                }
            }
        }

        // Trailing text without a terminal punctuation mark.
        let tail = text[start..].trim();
        if !tail.is_empty() {
            sentences.push(tail);
        }

        sentences
    }

    /// Splits a sentence that exceeds `max_tokens` by slicing the token array
    /// (O(1) tokenizations). Decoding token sub-slices preserves all content
    /// without data loss because every token ID maps back to exactly one string.
    fn split_oversized_sentence(&self, sentence: &str) -> Vec<String> {
        let tokens = self.tokenizer.encode_with_special_tokens(sentence);
        let mut sub_chunks = Vec::with_capacity(tokens.len() / self.max_tokens + 1);
        let mut token_offset = 0;

        while token_offset < tokens.len() {
            let end = (token_offset + self.max_tokens).min(tokens.len());
            let slice = &tokens[token_offset..end];
            let decoded = self.tokenizer.decode(slice.to_vec()).unwrap_or_default();
            if !decoded.is_empty() {
                sub_chunks.push(decoded);
            }
            token_offset = end;
        }

        sub_chunks
    }

    /// Merges sentences into chunks that stay within `max_tokens`, with overlap.
    /// Short paragraphs / sentences are merged together when they fit.
    fn merge_sentences_into_chunks(
        &self,
        sentences: &[&str],
    ) -> Result<Vec<String>, TextSplitterError> {
        if sentences.is_empty() {
            return Ok(Vec::new());
        }

        let mut chunks: Vec<String> = Vec::new();
        let mut idx = 0;

        while idx < sentences.len() {
            let sentence = sentences[idx];
            let sentence_tokens = self.count_tokens(sentence);

            // Sentence alone overflows the limit — split it internally.
            if sentence_tokens > self.max_tokens {
                let sub = self.split_oversized_sentence(sentence);
                chunks.extend(sub);
                idx += 1;
                continue;
            }

            // Build a chunk by accumulating sentences until we hit the limit.
            let mut current = String::from(sentence);
            let mut current_tokens = sentence_tokens;
            let chunk_start = idx;
            idx += 1;

            while idx < sentences.len() {
                let next = sentences[idx];
                let next_tokens = self.count_tokens(next);

                if next_tokens > self.max_tokens {
                    // Next sentence overflows on its own — flush current, let next
                    // iteration handle it.
                    break;
                }

                // +1 for the space separator we'd insert.
                let separator_tokens = if current.is_empty() { 0 } else { 1 };
                if current_tokens + separator_tokens + next_tokens > self.max_tokens {
                    break;
                }

                current.push(' ');
                current.push_str(next);
                current_tokens += separator_tokens + next_tokens;
                idx += 1;
            }

            let chunk_end = idx - 1; // inclusive last sentence index in this chunk
            chunks.push(current);

            // Compute overlap: walk backwards from chunk_end until we accumulate
            // enough overlap tokens, then rewind idx to replay those sentences.
            if idx < sentences.len() && self.overlap_tokens > 0 {
                let mut overlap_tokens_acc = 0;
                let mut overlap_start = chunk_end;

                while overlap_start > chunk_start && overlap_tokens_acc < self.overlap_tokens {
                    overlap_tokens_acc += self.count_tokens(sentences[overlap_start]);
                    if overlap_start > chunk_start {
                        overlap_start -= 1;
                    }
                }

                if overlap_start < chunk_end {
                    idx = overlap_start + 1;
                }
            }
        }

        Ok(chunks)
    }
}

#[async_trait]
impl TextSplitter for SemanticSplitter {
    async fn split(
        &self,
        text: &str,
        document_id: DocumentId,
        metadata: Option<Arc<DocumentMetadata>>,
    ) -> Result<Vec<Chunk>, TextSplitterError> {
        // Collect paragraphs while tracking their byte start positions in `text`
        // so offsets reflect the original document, not accumulated chunk lengths.
        let mut paragraphs: Vec<(usize, &str)> = Vec::new();
        let mut search_start = 0;

        for raw in text.split("\n\n") {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                // Locate where this trimmed slice begins inside `text`.
                let byte_pos = text[search_start..]
                    .find(trimmed)
                    .map(|rel| search_start + rel)
                    .unwrap_or(search_start);
                paragraphs.push((byte_pos, trimmed));
            }
            search_start += raw.len() + 2; // +2 for the "\n\n" delimiter
        }

        // Collect all sentences across paragraphs into a unified pool so that
        // short adjacent paragraphs can be merged into the same chunk.
        let mut all_sentences: Vec<(&str, usize)> = Vec::new(); // (sentence, byte_offset_in_text)
        for (para_offset, para) in &paragraphs {
            for sentence in self.split_into_sentences(para) {
                let byte_pos = text[*para_offset..]
                    .find(sentence)
                    .map(|rel| para_offset + rel)
                    .unwrap_or(*para_offset);
                all_sentences.push((sentence, byte_pos));
            }
        }

        let sentence_texts: Vec<&str> = all_sentences.iter().map(|(s, _)| *s).collect();
        let chunk_texts = self.merge_sentences_into_chunks(&sentence_texts)?;

        // Assign byte offsets by locating each chunk's first word in `text`.
        let mut all_chunks = Vec::with_capacity(chunk_texts.len());
        let mut search_from = 0usize;

        for chunk_text in chunk_texts {
            let first_word = chunk_text.split_whitespace().next().unwrap_or(&chunk_text);
            let offset = text[search_from..]
                .find(first_word)
                .map(|rel| search_from + rel)
                .unwrap_or(search_from);

            let chunk = match &metadata {
                Some(meta) => Chunk::with_metadata(
                    chunk_text.clone(),
                    document_id,
                    None,
                    offset,
                    Arc::clone(meta),
                ),
                None => Chunk::new(chunk_text.clone(), document_id, None, offset),
            };
            all_chunks.push(chunk);
            // Advance search cursor to avoid re-matching earlier text.
            search_from = offset;
        }

        Ok(all_chunks)
    }
}
