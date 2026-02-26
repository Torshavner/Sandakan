// @AI-BYPASS-LENGTH
use std::sync::Arc;

use async_trait::async_trait;

use crate::application::ports::{TextSplitter, TextSplitterError};
use crate::domain::{Chunk, DocumentId, DocumentMetadata, TranscriptSegment};

use super::SemanticSplitter;

static ABBREVIATIONS: &[&str] = &[
    "e.g", "i.e", "vs", "etc", "fig", "dr", "mr", "mrs", "prof", "st", "no", "vol", "dept",
    "approx", "inc", "ltd", "jr", "sr",
];

pub struct MarkdownSemanticSplitter {
    inner: SemanticSplitter,
}

impl MarkdownSemanticSplitter {
    pub fn new(max_tokens: usize, overlap_tokens: usize) -> Result<Self, TextSplitterError> {
        Ok(Self {
            inner: SemanticSplitter::new(max_tokens, overlap_tokens)?,
        })
    }
}

/// A structural unit extracted from markdown before sentence-level processing.
enum MarkdownBlock {
    Header(String),
    CodeFence(String),
    Table(String),
    Prose(String),
}

/// Parses markdown text into structural blocks via a line-by-line state machine.
fn parse_markdown_blocks(text: &str) -> Vec<MarkdownBlock> {
    let mut blocks: Vec<MarkdownBlock> = Vec::new();
    let mut in_code_fence = false;
    let mut fence_buf = String::new();
    let mut prose_buf = String::new();
    let mut table_buf = String::new();

    for line in text.lines() {
        let trimmed = line.trim();

        // Code fence toggle (``` or ~~~)
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            if in_code_fence {
                // Closing fence — include the delimiter and flush
                fence_buf.push('\n');
                fence_buf.push_str(line);
                blocks.push(MarkdownBlock::CodeFence(fence_buf.clone()));
                fence_buf.clear();
                in_code_fence = false;
            } else {
                flush_prose(&mut prose_buf, &mut blocks);
                flush_table(&mut table_buf, &mut blocks);
                fence_buf.push_str(line);
                in_code_fence = true;
            }
            continue;
        }

        if in_code_fence {
            fence_buf.push('\n');
            fence_buf.push_str(line);
            continue;
        }

        // Table detection: lines starting with `|`
        if trimmed.starts_with('|') {
            flush_prose(&mut prose_buf, &mut blocks);
            if !table_buf.is_empty() {
                table_buf.push('\n');
            }
            table_buf.push_str(line);
            continue;
        }

        // If we were in a table and hit a non-table line, flush the table
        flush_table(&mut table_buf, &mut blocks);

        // Header detection: lines starting with 1-6 `#` followed by whitespace
        if trimmed.starts_with('#') {
            let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
            if hashes <= 6 && trimmed.as_bytes().get(hashes) == Some(&b' ') {
                flush_prose(&mut prose_buf, &mut blocks);
                blocks.push(MarkdownBlock::Header(line.to_string()));
                continue;
            }
        }

        // Blank line separates prose paragraphs
        if trimmed.is_empty() {
            flush_prose(&mut prose_buf, &mut blocks);
            continue;
        }

        // Accumulate prose
        if !prose_buf.is_empty() {
            prose_buf.push(' ');
        }
        prose_buf.push_str(trimmed);
    }

    // Flush any remaining state
    if in_code_fence && !fence_buf.is_empty() {
        // Unclosed fence — treat as code fence anyway to preserve content
        blocks.push(MarkdownBlock::CodeFence(fence_buf));
    }
    flush_table(&mut table_buf, &mut blocks);
    flush_prose(&mut prose_buf, &mut blocks);

    blocks
}

fn flush_prose(buf: &mut String, blocks: &mut Vec<MarkdownBlock>) {
    if !buf.is_empty() {
        blocks.push(MarkdownBlock::Prose(std::mem::take(buf)));
    }
}

fn flush_table(buf: &mut String, blocks: &mut Vec<MarkdownBlock>) {
    if !buf.is_empty() {
        blocks.push(MarkdownBlock::Table(std::mem::take(buf)));
    }
}

/// Improved sentence splitter that handles numbered lists and abbreviations.
fn split_prose_into_sentences(prose: &str) -> Vec<String> {
    let mut sentences: Vec<String> = Vec::new();
    let mut start = 0;
    let bytes = prose.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let ch = bytes[i];

        if ch == b'.' || ch == b'!' || ch == b'?' {
            // Check if followed by whitespace or end-of-string (sentence boundary candidate)
            let is_followed_by_boundary = if i + 1 >= len {
                true
            } else {
                bytes[i + 1].is_ascii_whitespace()
            };

            if !is_followed_by_boundary {
                i += 1;
                continue;
            }

            if ch == b'.' && is_numbered_list_dot(prose, i) {
                i += 1;
                continue;
            }

            if ch == b'.' && is_abbreviation_dot(prose, i) {
                i += 1;
                continue;
            }

            let slice = prose[start..=i].trim();
            if !slice.is_empty() {
                sentences.push(slice.to_string());
            }
            start = i + 1;
        }

        i += 1;
    }

    let tail = prose[start..].trim();
    if !tail.is_empty() {
        sentences.push(tail.to_string());
    }

    sentences
}

/// Returns true if the period at `dot_pos` is part of a numbered list (e.g. `1. `, `12. `).
fn is_numbered_list_dot(text: &str, dot_pos: usize) -> bool {
    if dot_pos == 0 {
        return false;
    }
    let before = &text[..dot_pos];
    let word = before.split_whitespace().next_back().unwrap_or("");
    word.chars().all(|c| c.is_ascii_digit()) && !word.is_empty()
}

/// Returns true if the period at `dot_pos` follows a known abbreviation.
fn is_abbreviation_dot(text: &str, dot_pos: usize) -> bool {
    if dot_pos == 0 {
        return false;
    }
    // Walk backwards to find the word before the dot (may contain internal dots like "e.g")
    let before = &text[..dot_pos];
    let word_start = before
        .rfind(|c: char| c.is_whitespace())
        .map(|p| p + 1)
        .unwrap_or(0);
    let word = &before[word_start..];
    let lower = word.to_lowercase();
    ABBREVIATIONS.contains(&lower.as_str())
}

#[async_trait]
impl TextSplitter for MarkdownSemanticSplitter {
    async fn split(
        &self,
        text: &str,
        document_id: DocumentId,
        metadata: Option<Arc<DocumentMetadata>>,
    ) -> Result<Vec<Chunk>, TextSplitterError> {
        let blocks = parse_markdown_blocks(text);

        let mut units: Vec<String> = Vec::new();
        for block in blocks {
            match block {
                MarkdownBlock::Header(t) => units.push(t),
                MarkdownBlock::CodeFence(t) | MarkdownBlock::Table(t) => {
                    // Atomic unit — if oversized, merge_sentences_into_chunks
                    // will delegate to split_oversized_sentence as fallback.
                    units.push(t);
                }
                MarkdownBlock::Prose(t) => {
                    let sentences = split_prose_into_sentences(&t);
                    units.extend(sentences);
                }
            }
        }

        let unit_refs: Vec<&str> = units.iter().map(String::as_str).collect();
        let chunk_texts = self.inner.merge_sentences_into_chunks(&unit_refs)?;

        let mut all_chunks = Vec::with_capacity(chunk_texts.len());
        let mut search_from = 0usize;

        for chunk_text in chunk_texts {
            let first_word = chunk_text.split_whitespace().next().unwrap_or(&chunk_text);
            let offset = text[search_from..]
                .find(first_word)
                .map(|rel| search_from + rel)
                .unwrap_or(search_from);

            let chunk = match &metadata {
                Some(meta) => {
                    Chunk::with_metadata(chunk_text, document_id, None, offset, Arc::clone(meta))
                }
                None => Chunk::new(chunk_text, document_id, None, offset),
            };
            all_chunks.push(chunk);
            search_from = offset;
        }

        Ok(all_chunks)
    }

    async fn split_segments(
        &self,
        segments: &[TranscriptSegment],
        document_id: DocumentId,
        metadata: Option<Arc<DocumentMetadata>>,
    ) -> Result<Vec<Chunk>, TextSplitterError> {
        self.inner
            .split_segments_impl(segments, document_id, metadata)
    }
}
