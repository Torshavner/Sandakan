//! @AI: audio module routing map
//! - audio_decoder: FfmpegAudioDecoder implements the L2 AudioDecoder port via ffmpeg-sidecar
//!   (piped s16le PCM at 16kHz mono — no symphonia/rubato). check_ffmpeg_binary() validates
//!   the ffmpeg binary is on $PATH; called at startup only when provider == Local.
//! - candle_whisper_engine: Local Whisper transcription via Candle ML (Metal/CPU). Accepts
//!   Arc<dyn AudioDecoder> at construction; calls decoder.decode() then runs mel-spectrogram
//!   → Whisper inference loop.
//! - openai_whisper_engine: Remote transcription via OpenAI /v1/audio/transcriptions API.
//!   Sends raw &[u8] bytes directly — no AudioDecoder needed. Configurable base_url.
//! - azure_whisper_engine: Remote transcription via Azure OpenAI Whisper. Sends raw &[u8]
//!   bytes to {endpoint}/openai/deployments/{deployment}/audio/transcriptions?api-version={}
//!   with api-key header auth. No AudioDecoder needed.
//! - transcription_engine_factory: Creates Arc<dyn TranscriptionEngine> by TranscriptionProvider
//!   enum (Local | OpenAi | Azure). Entry point for composition root.

pub mod audio_decoder;
mod azure_whisper_engine;
mod candle_whisper_engine;
mod openai_whisper_engine;
mod transcription_engine_factory;

pub use audio_decoder::{FfmpegAudioDecoder, check_ffmpeg_binary};
pub use azure_whisper_engine::AzureWhisperEngine;
pub use candle_whisper_engine::CandleWhisperEngine;
pub use openai_whisper_engine::OpenAiWhisperEngine;
pub use transcription_engine_factory::{TranscriptionEngineFactory, TranscriptionProvider};
