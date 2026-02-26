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
    max_tokens: usize,
}

impl MarkdownSemanticSplitter {
    pub fn new(max_tokens: usize, overlap_tokens: usize) -> Result<Self, TextSplitterError> {
        Ok(Self {
            inner: SemanticSplitter::new(max_tokens, overlap_tokens)?,
            max_tokens,
        })
    }
}

/// A structural unit extracted from markdown before sentence-level processing.
enum MarkdownBlock {
    Header(usize, String),
    CodeFence(String),
    Table(String),
    Prose(String),
}

/// A section groups a header with its content blocks.
struct Section {
    header: Option<String>,
    blocks: Vec<MarkdownBlock>,
}

/// Groups parsed blocks into sections: each header starts a new section.
fn group_into_sections(blocks: Vec<MarkdownBlock>) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut current = Section {
        header: None,
        blocks: Vec::new(),
    };
    let mut ancestor_stack: Vec<(usize, String)> = Vec::new();

    for block in blocks {
        match block {
            MarkdownBlock::Header(level, text) => {
                // Only push if the section has content; header-only sections are
                // dropped — their header is already in the ancestor stack and will
                // appear as part of the breadcrumb on the next content section.
                if !current.blocks.is_empty() {
                    sections.push(current);
                }
                // Pop headers at same or deeper level (they're no longer ancestors)
                while ancestor_stack.last().is_some_and(|(l, _)| *l >= level) {
                    ancestor_stack.pop();
                }
                ancestor_stack.push((level, text));
                // Build breadcrumb from full ancestor stack
                let breadcrumb = ancestor_stack
                    .iter()
                    .map(|(_, h)| h.as_str())
                    .collect::<Vec<_>>()
                    .join(" > ");
                current = Section {
                    header: Some(breadcrumb),
                    blocks: Vec::new(),
                };
            }
            other => {
                current.blocks.push(other);
            }
        }
    }

    if !current.blocks.is_empty() {
        sections.push(current);
    }

    sections
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

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            if in_code_fence {
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

        if trimmed.starts_with('|') {
            flush_prose(&mut prose_buf, &mut blocks);
            if !table_buf.is_empty() {
                table_buf.push('\n');
            }
            table_buf.push_str(line);
            continue;
        }

        flush_table(&mut table_buf, &mut blocks);

        if trimmed.starts_with('#') {
            let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
            if hashes <= 6 && trimmed.as_bytes().get(hashes) == Some(&b' ') {
                flush_prose(&mut prose_buf, &mut blocks);
                blocks.push(MarkdownBlock::Header(hashes, line.to_string()));
                continue;
            }
        }

        if trimmed.is_empty() {
            flush_prose(&mut prose_buf, &mut blocks);
            continue;
        }

        if !prose_buf.is_empty() {
            prose_buf.push(' ');
        }
        prose_buf.push_str(trimmed);
    }

    if in_code_fence && !fence_buf.is_empty() {
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

fn is_numbered_list_dot(text: &str, dot_pos: usize) -> bool {
    if dot_pos == 0 {
        return false;
    }
    let before = &text[..dot_pos];
    let word = before.split_whitespace().next_back().unwrap_or("");
    word.chars().all(|c| c.is_ascii_digit()) && !word.is_empty()
}

fn is_abbreviation_dot(text: &str, dot_pos: usize) -> bool {
    if dot_pos == 0 {
        return false;
    }
    let before = &text[..dot_pos];
    let word_start = before
        .rfind(|c: char| c.is_whitespace())
        .map(|p| p + 1)
        .unwrap_or(0);
    let word = &before[word_start..];
    let lower = word.to_lowercase();
    ABBREVIATIONS.contains(&lower.as_str())
}

/// Collects text units from a block for token-budgeted chunking.
///
/// Prose: kept as a single atomic string (paragraph atomicity).
/// Only sentence-split if the paragraph exceeds `max_tokens`.
/// CodeFence / Table: atomic. Split via `split_oversized_sentence` if oversized.
fn block_to_units(
    block: &MarkdownBlock,
    splitter: &SemanticSplitter,
    max_tokens: usize,
) -> Vec<String> {
    match block {
        MarkdownBlock::Header(_, _) => Vec::new(),
        MarkdownBlock::CodeFence(t) | MarkdownBlock::Table(t) => {
            if splitter.count_tokens(t) > max_tokens {
                splitter.split_oversized_sentence(t)
            } else {
                vec![t.clone()]
            }
        }
        MarkdownBlock::Prose(t) => {
            if splitter.count_tokens(t) > max_tokens {
                let sentences = split_prose_into_sentences(t);
                let mut result = Vec::new();
                for s in sentences {
                    if splitter.count_tokens(&s) > max_tokens {
                        result.extend(splitter.split_oversized_sentence(&s));
                    } else {
                        result.push(s);
                    }
                }
                result
            } else {
                vec![t.clone()]
            }
        }
    }
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
        let sections = group_into_sections(blocks);

        if sections.is_empty() {
            return Ok(Vec::new());
        }

        let chunk_texts = self.build_section_chunks(&sections);

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

impl MarkdownSemanticSplitter {
    /// Builds chunk text strings from sections with header propagation and hard boundaries.
    fn build_section_chunks(&self, sections: &[Section]) -> Vec<String> {
        let mut chunks: Vec<String> = Vec::new();
        let mut current_text = String::new();
        let mut current_tokens: usize = 0;

        for section in sections {
            // Hard boundary: finalize any in-progress chunk when a new header section starts
            if section.header.is_some() && !current_text.is_empty() {
                chunks.push(std::mem::take(&mut current_text));
                current_tokens = 0;
            }

            let header_prefix = section.header.as_deref().unwrap_or("");
            let header_tokens = if header_prefix.is_empty() {
                0
            } else {
                self.inner.count_tokens(header_prefix)
            };

            // Start first chunk of this section with the header
            if !header_prefix.is_empty() {
                current_text.push_str(header_prefix);
                current_tokens = header_tokens;
            }

            for block in &section.blocks {
                let units = block_to_units(block, &self.inner, self.max_tokens);

                for unit in units {
                    let unit_tokens = self.inner.count_tokens(&unit);
                    let separator_tokens = if current_text.is_empty() { 0 } else { 1 };

                    if current_tokens + separator_tokens + unit_tokens > self.max_tokens
                        && !current_text.is_empty()
                    {
                        chunks.push(std::mem::take(&mut current_text));
                        current_tokens = 0;

                        // Header propagation: continuation chunks get the header prefix
                        if !header_prefix.is_empty() {
                            current_text.push_str(header_prefix);
                            current_tokens = header_tokens;
                        }
                    }

                    if !current_text.is_empty() {
                        current_text.push('\n');
                        current_tokens += 1;
                    }
                    current_text.push_str(&unit);
                    current_tokens += unit_tokens;
                }
            }
        }

        if !current_text.is_empty() {
            chunks.push(current_text);
        }

        chunks
    }
}
