use std::sync::LazyLock;
use tiktoken_rs::CoreBPE;

static TOKENIZER: LazyLock<CoreBPE> = LazyLock::new(|| {
    tiktoken_rs::cl100k_base().expect("Failed to initialize cl100k_base tokenizer")
});

pub fn count_tokens(text: &str) -> usize {
    TOKENIZER.encode_with_special_tokens(text).len()
}
