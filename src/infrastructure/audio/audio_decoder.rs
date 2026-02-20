use std::io::Read;

use ffmpeg_sidecar::command::FfmpegCommand;

use crate::application::ports::{AudioDecoder, AudioDecoderError, TranscriptionError};

pub fn check_ffmpeg_binary() -> Result<(), TranscriptionError> {
    let mut child = FfmpegCommand::new().arg("-version").spawn().map_err(|e| {
        TranscriptionError::DecodingFailed(format!("ffmpeg binary not found in $PATH: {}", e))
    })?;

    let status = child.wait().map_err(|e| {
        TranscriptionError::DecodingFailed(format!("ffmpeg version check failed: {}", e))
    })?;

    if status.success() {
        Ok(())
    } else {
        Err(TranscriptionError::DecodingFailed(
            "ffmpeg binary returned non-zero exit code during version check".to_string(),
        ))
    }
}

pub struct FfmpegAudioDecoder;

impl AudioDecoder for FfmpegAudioDecoder {
    fn decode(&self, data: &[u8]) -> Result<Vec<f32>, AudioDecoderError> {
        let input = tempfile::Builder::new()
            .suffix(".media")
            .tempfile()
            .map_err(|e| AudioDecoderError::DecodingFailed(format!("tempfile: {}", e)))?;

        std::fs::write(input.path(), data)
            .map_err(|e| AudioDecoderError::DecodingFailed(format!("write temp: {}", e)))?;

        let mut child = FfmpegCommand::new()
            .args([
                "-y",
                "-i",
                input.path().to_str().unwrap_or_default(),
                "-vn",
                "-ar",
                "16000",
                "-ac",
                "1",
                "-f",
                "s16le",
                "pipe:1",
            ])
            .spawn()
            .map_err(|e| {
                AudioDecoderError::DecodingFailed(format!("ffmpeg spawn failed: {}", e))
            })?;

        let mut stdout = child.take_stdout().ok_or_else(|| {
            AudioDecoderError::DecodingFailed("ffmpeg stdout unavailable".to_string())
        })?;

        let mut raw_bytes: Vec<u8> = Vec::new();
        stdout.read_to_end(&mut raw_bytes).map_err(|e| {
            AudioDecoderError::DecodingFailed(format!("reading ffmpeg stdout: {}", e))
        })?;

        let status = child
            .wait()
            .map_err(|e| AudioDecoderError::DecodingFailed(format!("ffmpeg wait: {}", e)))?;

        if !status.success() {
            return Err(AudioDecoderError::DecodingFailed(
                "ffmpeg exited with non-zero status during audio extraction".to_string(),
            ));
        }

        if raw_bytes.is_empty() {
            return Err(AudioDecoderError::DecodingFailed(
                "no audio samples decoded: ffmpeg produced no output".to_string(),
            ));
        }

        // raw_bytes is s16le: pairs of bytes per sample, little-endian signed 16-bit
        let pcm: Vec<f32> = raw_bytes
            .chunks_exact(2)
            .map(|b| {
                let sample = i16::from_le_bytes([b[0], b[1]]);
                sample as f32 / i16::MAX as f32
            })
            .collect();

        tracing::debug!(
            samples = pcm.len(),
            duration_secs = pcm.len() as f32 / 16_000.0,
            "Audio decoded to 16kHz mono PCM via ffmpeg-sidecar"
        );

        Ok(pcm)
    }
}
