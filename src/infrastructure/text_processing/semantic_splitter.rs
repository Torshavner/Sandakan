use async_trait::async_trait;
use tiktoken_rs::cl100k_base;
use unicode_segmentation::UnicodeSegmentation;

use crate::application::ports::{TextSplitter, TextSplitterError};
use crate::domain::{Chunk, DocumentId};

pub struct SemanticSplitter {
    max_tokens: usize,
    overlap_tokens: usize,
}

impl SemanticSplitter {
    pub fn new(max_tokens: usize, overlap_tokens: usize) -> Self {
        Self {
            max_tokens,
            overlap_tokens,
        }
    }

    fn count_tokens(&self, text: &str) -> Result<usize, TextSplitterError> {
        let bpe = cl100k_base().map_err(|e| {
            TextSplitterError::TokenizationFailed(format!("Failed to load tokenizer: {}", e))
        })?;
        Ok(bpe.encode_with_special_tokens(text).len())
    }

    fn split_into_sentences(&self, text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut current = String::new();

        for grapheme in text.graphemes(true) {
            current.push_str(grapheme);

            if grapheme == "." || grapheme == "!" || grapheme == "?" {
                if let Some(next_char) = text.chars().nth(current.len()) {
                    if next_char.is_whitespace() || current.len() == text.len() {
                        sentences.push(current.trim().to_string());
                        current.clear();
                    }
                } else {
                    sentences.push(current.trim().to_string());
                    current.clear();
                }
            }
        }

        if !current.trim().is_empty() {
            sentences.push(current.trim().to_string());
        }

        sentences.into_iter().filter(|s| !s.is_empty()).collect()
    }

    fn split_oversized_sentence(&self, sentence: &str) -> Result<Vec<String>, TextSplitterError> {
        let chars: Vec<char> = sentence.chars().collect();
        let mut sub_chunks = Vec::new();
        let mut offset = 0;

        while offset < chars.len() {
            let remaining = chars.len() - offset;
            let mut low = 1;
            let mut high = remaining;
            let mut best_len = 1;

            while low <= high {
                let mid = (low + high) / 2;
                let test_text: String = chars[offset..offset + mid].iter().collect();
                let token_count = self.count_tokens(&test_text)?;

                if token_count <= self.max_tokens {
                    best_len = mid;
                    low = mid + 1;
                } else {
                    high = mid - 1;
                }
            }

            let chunk_text: String = chars[offset..offset + best_len].iter().collect();
            sub_chunks.push(chunk_text);

            offset += best_len;
        }

        Ok(sub_chunks)
    }

    fn merge_sentences_into_chunks(
        &self,
        sentences: Vec<String>,
    ) -> Result<Vec<String>, TextSplitterError> {
        if sentences.is_empty() {
            return Ok(Vec::new());
        }

        let mut chunks = Vec::new();
        let mut start_idx = 0;

        while start_idx < sentences.len() {
            let mut current_chunk = String::new();
            let mut current_tokens = 0;
            let mut end_idx = start_idx;

            for (idx, sentence) in sentences.iter().enumerate().skip(start_idx) {
                let sentence_tokens = self.count_tokens(sentence)?;

                if sentence_tokens > self.max_tokens {
                    if !current_chunk.is_empty() {
                        chunks.push(std::mem::take(&mut current_chunk));
                    }

                    let sub_chunks = self.split_oversized_sentence(sentence)?;
                    chunks.extend(sub_chunks);

                    end_idx = idx;
                    start_idx = idx + 1;

                    continue;
                }

                if current_tokens + sentence_tokens > self.max_tokens && !current_chunk.is_empty() {
                    break;
                }

                if !current_chunk.is_empty() {
                    current_chunk.push(' ');
                }
                current_chunk.push_str(sentence);
                current_tokens += sentence_tokens;
                end_idx = idx;
            }

            if !current_chunk.is_empty() {
                chunks.push(current_chunk);
            }

            if start_idx <= end_idx {
                let mut overlap_idx = end_idx;
                let mut overlap_tokens = 0;

                while overlap_idx > start_idx && overlap_tokens < self.overlap_tokens {
                    let sentence_tokens = self.count_tokens(&sentences[overlap_idx])?;
                    overlap_tokens += sentence_tokens;
                    if overlap_idx > 0 {
                        overlap_idx -= 1;
                    } else {
                        break;
                    }
                }

                start_idx = if overlap_idx < end_idx {
                    overlap_idx + 1
                } else {
                    end_idx + 1
                };
            }

            if start_idx <= end_idx && end_idx == sentences.len() - 1 {
                break;
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
    ) -> Result<Vec<Chunk>, TextSplitterError> {
        let paragraphs: Vec<&str> = text
            .split("\n\n")
            .filter(|p| !p.trim().is_empty())
            .collect();

        let mut all_chunks = Vec::new();
        let mut global_offset = 0;

        for paragraph in paragraphs {
            let sentences = self.split_into_sentences(paragraph);
            let chunk_texts = self.merge_sentences_into_chunks(sentences)?;

            for chunk_text in chunk_texts {
                let offset = global_offset;
                all_chunks.push(Chunk::new(chunk_text.clone(), document_id, None, offset));
                global_offset += chunk_text.len();
            }
        }

        Ok(all_chunks)
    }
}
