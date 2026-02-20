//! @AI: text_processing module routing map
//! - azure_doc_intel_adapter    -> Cloud PDF adapter: submits bytes to Azure Document Intelligence,
//!   polls Operation-Location, returns native Markdown.
//! - composite_file_loader      -> Routes FileLoader calls by ContentType to registered adapters.
//! - extractor_factory          -> Creates Arc<dyn FileLoader> for PDF extraction via ExtractorProvider enum
//!   (local_vlm | lm_studio | azure). Entry point for composition root.
//! - lm_studio_vlm_pdf_adapter  -> HTTP PDF adapter: rasterizes pages via pdf_rasterizer, POSTs each
//!   page as base64 PNG to LM Studio /v1/chat/completions (OpenAI vision format), returns Markdown.
//! - local_vlm_pdf_adapter      -> Embedded PDF adapter: rasterizes pages via pdf_rasterizer, runs
//!   moondream VLM inference via candle, returns Markdown. parse_shard_names handles both single-shard
//!   and multi-shard HF weight layouts (testable without network I/O).
//! - pdf_rasterizer             -> Shared pdfium rasterization: converts PDF bytes to per-page PNG
//!   buffers at RENDER_DPI, capped at MAX_PAGES_DUE_TO_RAM_USAGE. Used by both VLM PDF adapters.
//! - mock_file_loader           -> In-memory stub implementing FileLoader for offline tests.
//! - plain_text_adapter         -> FileLoader for text/plain documents (no-op passthrough).
//! - recursive_character_splitter -> Fixed-size chunking TextSplitter.
//! - semantic_splitter          -> Token-aware semantic TextSplitter.
//! - text_sanitizer             -> Normalises and de-hyphens raw extracted text.
//! - text_splitter_factory      -> Creates Arc<dyn TextSplitter> by ChunkingStrategy enum.

mod azure_doc_intel_adapter;
mod composite_file_loader;
mod extractor_factory;
mod lm_studio_vlm_pdf_adapter;
mod local_vlm_pdf_adapter;
mod mock_file_loader;
mod pdf_rasterizer;
mod plain_text_adapter;
mod recursive_character_splitter;
mod semantic_splitter;
mod text_sanitizer;
mod text_splitter_factory;

pub use azure_doc_intel_adapter::AnalyzeResponse;
pub use azure_doc_intel_adapter::AnalyzeResult;
pub use azure_doc_intel_adapter::AzureDocIntelAdapter;

pub use composite_file_loader::CompositeFileLoader;
pub use extractor_factory::{ExtractorFactory, ExtractorFactoryError};
pub use lm_studio_vlm_pdf_adapter::LmStudioVlmPdfAdapter;
pub use local_vlm_pdf_adapter::LocalVlmPdfAdapter;
pub use local_vlm_pdf_adapter::parse_shard_names;
pub use mock_file_loader::MockFileLoader;
pub use plain_text_adapter::PlainTextAdapter;
pub use recursive_character_splitter::RecursiveCharacterSplitter;
pub use semantic_splitter::SemanticSplitter;
pub use text_sanitizer::sanitize_extracted_text;
pub use text_splitter_factory::TextSplitterFactory;

pub use local_vlm_pdf_adapter::EXTRACTION_TIMEOUT;
pub use local_vlm_pdf_adapter::MAX_PAGES_DUE_TO_RAM_USAGE;
pub use local_vlm_pdf_adapter::OCR_PROMPT;
pub use local_vlm_pdf_adapter::RENDER_DPI;
