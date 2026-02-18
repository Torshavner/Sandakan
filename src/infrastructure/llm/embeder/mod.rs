pub mod embedder_factory;
pub mod local_candle_embedder;
mod mock_embedder;
pub mod openai_embedder;

pub use embedder_factory::{EmbedderFactory, EmbedderFactoryError};
pub use local_candle_embedder::LocalCandleEmbedder;
pub use mock_embedder::MockEmbedder;
pub use openai_embedder::OpenAiEmbedder;
