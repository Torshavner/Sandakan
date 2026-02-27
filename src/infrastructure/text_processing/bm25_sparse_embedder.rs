use std::collections::HashMap;

use async_trait::async_trait;
use unicode_segmentation::UnicodeSegmentation;

use crate::application::ports::{EmbedderError, SparseEmbedder};
use crate::domain::SparseEmbedding;

const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
    "do", "does", "did", "will", "would", "could", "should", "may", "might", "shall", "can",
    "need", "dare", "ought", "in", "on", "at", "to", "for", "of", "with", "by", "from", "as",
    "into", "through", "during", "before", "after", "above", "below", "and", "or", "but", "if",
    "then", "that", "this", "it", "its", "not", "no", "nor", "so", "yet", "both", "either",
    "neither",
];

pub struct Bm25SparseEmbedder;

impl Bm25SparseEmbedder {
    pub fn new() -> Self {
        Self
    }

    fn tokenize(text: &str) -> Vec<String> {
        text.unicode_words()
            .map(|w| w.to_lowercase())
            .filter(|w| w.len() > 1 && !STOP_WORDS.contains(&w.as_str()))
            .collect()
    }

    fn fnv1a(token: &str) -> u32 {
        const FNV_PRIME: u32 = 16_777_619;
        const FNV_OFFSET: u32 = 2_166_136_261;
        token.bytes().fold(FNV_OFFSET, |acc, b| {
            (acc ^ b as u32).wrapping_mul(FNV_PRIME)
        })
    }

    fn compute_sparse(text: &str) -> SparseEmbedding {
        let tokens = Self::tokenize(text);
        if tokens.is_empty() {
            return SparseEmbedding::new(vec![]);
        }

        let total = tokens.len() as f32;
        let mut counts: HashMap<u32, u32> = HashMap::new();
        for token in &tokens {
            *counts.entry(Self::fnv1a(token)).or_insert(0) += 1;
        }

        let pairs: Vec<(u32, f32)> = counts
            .into_iter()
            .map(|(idx, count)| (idx, count as f32 / total))
            .collect();

        SparseEmbedding::new(pairs)
    }
}

impl Default for Bm25SparseEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SparseEmbedder for Bm25SparseEmbedder {
    async fn embed_sparse(&self, text: &str) -> Result<SparseEmbedding, EmbedderError> {
        Ok(Self::compute_sparse(text))
    }

    async fn embed_sparse_batch(
        &self,
        texts: &[&str],
    ) -> Result<Vec<SparseEmbedding>, EmbedderError> {
        Ok(texts.iter().map(|t| Self::compute_sparse(t)).collect())
    }
}
