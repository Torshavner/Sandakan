use std::io::Cursor;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::application::ports::TranscriptionError;

const TARGET_SAMPLE_RATE: u32 = 16_000;

pub fn decode_audio_to_pcm(data: &[u8]) -> Result<Vec<f32>, TranscriptionError> {
    let cursor = Cursor::new(data.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let hint = Hint::new();
    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();
    let decoder_opts = DecoderOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| TranscriptionError::DecodingFailed(format!("probe: {}", e)))?;

    let mut format = probed.format;

    let track = format
        .default_track()
        .ok_or_else(|| TranscriptionError::DecodingFailed("no audio track found".to_string()))?;

    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let source_rate = codec_params
        .sample_rate
        .ok_or_else(|| TranscriptionError::DecodingFailed("unknown sample rate".to_string()))?;
    let channels = codec_params.channels.map(|c| c.count()).unwrap_or(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &decoder_opts)
        .map_err(|e| TranscriptionError::DecodingFailed(format!("codec: {}", e)))?;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                return Err(TranscriptionError::DecodingFailed(format!("packet: {}", e)));
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(e)) => {
                tracing::warn!(error = %e, "Skipping corrupt audio frame");
                continue;
            }
            Err(e) => {
                return Err(TranscriptionError::DecodingFailed(format!("decode: {}", e)));
            }
        };

        let spec = *decoded.spec();
        let num_frames = decoded.frames();
        if num_frames == 0 {
            continue;
        }

        let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        let samples = sample_buf.samples();

        // Downmix to mono if multi-channel
        if channels > 1 {
            for frame in samples.chunks(channels) {
                let mono: f32 = frame.iter().sum::<f32>() / channels as f32;
                all_samples.push(mono);
            }
        } else {
            all_samples.extend_from_slice(samples);
        }
    }

    if all_samples.is_empty() {
        return Err(TranscriptionError::DecodingFailed(
            "no audio samples decoded".to_string(),
        ));
    }

    // Resample to 16kHz if needed
    if source_rate != TARGET_SAMPLE_RATE {
        all_samples = resample(&all_samples, source_rate, TARGET_SAMPLE_RATE)?;
    }

    tracing::debug!(
        samples = all_samples.len(),
        duration_secs = all_samples.len() as f32 / TARGET_SAMPLE_RATE as f32,
        "Audio decoded to 16kHz mono PCM"
    );

    Ok(all_samples)
}

fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>, TranscriptionError> {
    use rubato::{
        Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
    };

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let ratio = to_rate as f64 / from_rate as f64;
    let chunk_size = 1024;

    let mut resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, chunk_size, 1)
        .map_err(|e| TranscriptionError::DecodingFailed(format!("resampler init: {}", e)))?;

    let mut output = Vec::with_capacity((samples.len() as f64 * ratio) as usize + chunk_size);

    for chunk in samples.chunks(chunk_size) {
        let input = if chunk.len() < chunk_size {
            let mut padded = chunk.to_vec();
            padded.resize(chunk_size, 0.0);
            padded
        } else {
            chunk.to_vec()
        };

        let result = resampler
            .process(&[input], None)
            .map_err(|e| TranscriptionError::DecodingFailed(format!("resample: {}", e)))?;

        if let Some(channel) = result.first() {
            output.extend_from_slice(channel);
        }
    }

    // Trim to approximate expected length
    let expected_len = (samples.len() as f64 * ratio) as usize;
    output.truncate(expected_len);

    Ok(output)
}
