/// A timed speech segment produced by a transcription engine.
///
/// Maps 1-to-1 with a Whisper output segment (or equivalent). The `start_time`
/// and `end_time` are in seconds relative to the beginning of the media file.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptSegment {
    pub text: String,
    pub start_time: f32,
    pub end_time: f32,
}

impl TranscriptSegment {
    pub fn new(text: impl Into<String>, start_time: f32, end_time: f32) -> Self {
        Self {
            text: text.into(),
            start_time,
            end_time,
        }
    }

    /// Returns the full plain text across all segments joined by a single space.
    pub fn merge_text(segments: &[TranscriptSegment]) -> String {
        segments
            .iter()
            .map(|s| s.text.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}
