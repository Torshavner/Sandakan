mod mock_embedder;
pub mod embedder_factory;
pub mod openai_embedder;
pub mod local_candle_embedder;

pub use embedder_factory::{EmbedderFactory, EmbedderFactoryError};
pub use local_candle_embedder::LocalCandleEmbedder;
pub use openai_embedder::OpenAiEmbedder;
pub use mock_embedder::MockEmbedder;