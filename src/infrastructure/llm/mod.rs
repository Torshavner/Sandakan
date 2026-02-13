mod embedder_factory;
mod local_candle_embedder;
mod openai_client;
mod openai_embedder;
mod streaming_client;

pub use embedder_factory::{EmbedderFactory, EmbedderFactoryError};
pub use local_candle_embedder::LocalCandleEmbedder;
pub use openai_client::OpenAiClient;
pub use openai_embedder::OpenAiEmbedder;
pub use streaming_client::{StreamingLlmClient, create_streaming_llm_client};
