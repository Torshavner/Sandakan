use sandakan::application::ports::AudioDecoder;
use sandakan::infrastructure::audio::audio_decoder::{FfmpegAudioDecoder, check_ffmpeg_binary};

fn build_wav(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let num_samples = samples.len() as u32;
    let byte_rate = sample_rate * 2;
    let data_size = num_samples * 2;
    let file_size = 36 + data_size;

    let mut wav = Vec::with_capacity(44 + data_size as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    for &s in samples {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    wav
}

fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn given_wav_bytes_when_decoding_via_ffmpeg_then_returns_pcm_samples() {
    if !ffmpeg_available() {
        return;
    }

    let samples: Vec<i16> = vec![0i16; 1600];
    let wav = build_wav(16_000, &samples);
    let decoder = FfmpegAudioDecoder;

    let result = decoder.decode(&wav);

    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());
}

#[test]
fn given_wav_at_44100hz_when_decoding_via_ffmpeg_then_resamples_to_16khz_output() {
    if !ffmpeg_available() {
        return;
    }

    let samples: Vec<i16> = vec![0i16; 4410];
    let wav = build_wav(44_100, &samples);
    let decoder = FfmpegAudioDecoder;

    let result = decoder.decode(&wav);

    assert!(result.is_ok());
    let pcm = result.unwrap();
    assert!(!pcm.is_empty());
    // ffmpeg resamples to 16kHz: 4410 samples @ 44100Hz ≈ 0.1s → ~1600 samples @ 16kHz
    assert!(
        pcm.len() < 4410,
        "output should be fewer samples than 44.1kHz input"
    );
}

#[test]
fn given_mp4_bytes_when_decoding_via_ffmpeg_then_returns_pcm_samples() {
    if !ffmpeg_available() {
        return;
    }

    let wav = build_wav(44_100, &vec![0i16; 4410]);

    let input_wav = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let output_mp4 = tempfile::Builder::new().suffix(".mp4").tempfile().unwrap();
    std::fs::write(input_wav.path(), &wav).unwrap();

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            input_wav.path().to_str().unwrap(),
            "-c:a",
            "aac",
            output_mp4.path().to_str().unwrap(),
        ])
        .output()
        .expect("ffmpeg must be installed");

    if !status.status.success() {
        return;
    }

    let mp4_bytes = std::fs::read(output_mp4.path()).unwrap();
    let decoder = FfmpegAudioDecoder;
    let result = decoder.decode(&mp4_bytes);

    assert!(result.is_ok());
    assert!(!result.unwrap().is_empty());
}

#[test]
fn given_corrupted_bytes_when_decoding_then_returns_decoding_error() {
    if !ffmpeg_available() {
        return;
    }

    let garbage = vec![0xFFu8; 128];
    let decoder = FfmpegAudioDecoder;

    let result = decoder.decode(&garbage);

    assert!(matches!(
        result,
        Err(sandakan::application::ports::AudioDecoderError::DecodingFailed(_))
    ));
}

#[test]
fn given_empty_bytes_when_decoding_then_returns_decoding_error() {
    if !ffmpeg_available() {
        return;
    }

    let decoder = FfmpegAudioDecoder;
    let result = decoder.decode(&[]);

    assert!(matches!(
        result,
        Err(sandakan::application::ports::AudioDecoderError::DecodingFailed(_))
    ));
}

#[test]
fn given_ffmpeg_in_path_when_checking_binary_then_returns_ok() {
    if !ffmpeg_available() {
        return;
    }

    let result = check_ffmpeg_binary();

    assert!(result.is_ok());
}
