use sandakan::application::ports::SourceChunk;
use sandakan::application::ports::TextSplitter;
use sandakan::domain::{ContentType, DocumentId, DocumentMetadata, TranscriptSegment};
use sandakan::infrastructure::text_processing::SemanticSplitter;
use std::sync::Arc;

// ─── SourceChunk::timestamped_url ────────────────────────────────────────────

#[test]
fn given_source_chunk_without_url_when_timestamped_url_called_then_returns_none() {
    let chunk = SourceChunk {
        text: "some text".to_string(),
        page: None,
        score: 0.9,
        title: None,
        source_url: None,
        content_type: None,
        start_time: Some(45.0),
    };
    assert_eq!(chunk.timestamped_url(), None);
}

#[test]
fn given_source_chunk_with_url_and_no_start_time_when_timestamped_url_called_then_returns_base_url()
{
    let chunk = SourceChunk {
        text: "some text".to_string(),
        page: None,
        score: 0.9,
        title: None,
        source_url: Some("https://example.com/lecture.mp4".to_string()),
        content_type: None,
        start_time: None,
    };
    assert_eq!(
        chunk.timestamped_url(),
        Some("https://example.com/lecture.mp4".to_string())
    );
}

#[test]
fn given_source_chunk_with_url_and_start_time_when_timestamped_url_called_then_appends_t_param() {
    let chunk = SourceChunk {
        text: "some text".to_string(),
        page: None,
        score: 0.9,
        title: None,
        source_url: Some("https://example.com/lecture.mp4".to_string()),
        content_type: None,
        start_time: Some(1045.3),
    };
    assert_eq!(
        chunk.timestamped_url(),
        Some("https://example.com/lecture.mp4?t=1045s".to_string())
    );
}

#[test]
fn given_source_chunk_with_query_string_url_and_start_time_when_timestamped_url_called_then_uses_ampersand()
 {
    let chunk = SourceChunk {
        text: "some text".to_string(),
        page: None,
        score: 0.9,
        title: None,
        source_url: Some("https://youtube.com/watch?v=XYZ".to_string()),
        content_type: None,
        start_time: Some(1045.0),
    };
    assert_eq!(
        chunk.timestamped_url(),
        Some("https://youtube.com/watch?v=XYZ&t=1045s".to_string())
    );
}

#[test]
fn given_start_time_with_fractional_seconds_when_timestamped_url_called_then_rounds_to_nearest_second()
 {
    let chunk = SourceChunk {
        text: "text".to_string(),
        page: None,
        score: 0.9,
        title: None,
        source_url: Some("https://example.com/video".to_string()),
        content_type: None,
        start_time: Some(30.7),
    };
    // 30.7 rounds to 31
    assert_eq!(
        chunk.timestamped_url(),
        Some("https://example.com/video?t=31s".to_string())
    );
}

// ─── SemanticSplitter::split_segments ────────────────────────────────────────

#[tokio::test]
async fn given_empty_segments_when_split_segments_called_then_returns_empty_vec() {
    let splitter = SemanticSplitter::new(256, 32).unwrap();
    let doc_id = DocumentId::new();

    let chunks = splitter.split_segments(&[], doc_id, None).await.unwrap();

    assert!(chunks.is_empty());
}

#[tokio::test]
async fn given_single_segment_when_split_segments_called_then_chunk_carries_start_time() {
    let splitter = SemanticSplitter::new(256, 0).unwrap();
    let doc_id = DocumentId::new();
    let segments = vec![TranscriptSegment::new(
        "The mitochondria is the powerhouse of the cell.",
        42.0,
        47.5,
    )];

    let chunks = splitter
        .split_segments(&segments, doc_id, None)
        .await
        .unwrap();

    assert_eq!(chunks.len(), 1);
    assert!((chunks[0].start_time.unwrap() - 42.0).abs() < f32::EPSILON);
    assert_eq!(
        chunks[0].text,
        "The mitochondria is the powerhouse of the cell."
    );
}

#[tokio::test]
async fn given_many_segments_fitting_one_chunk_when_split_segments_called_then_start_time_is_first_segment()
 {
    let splitter = SemanticSplitter::new(512, 0).unwrap();
    let doc_id = DocumentId::new();
    let segments = vec![
        TranscriptSegment::new("First sentence.", 10.0, 13.0),
        TranscriptSegment::new("Second sentence.", 13.5, 16.0),
        TranscriptSegment::new("Third sentence.", 16.2, 19.0),
    ];

    let chunks = splitter
        .split_segments(&segments, doc_id, None)
        .await
        .unwrap();

    // All three short segments should fit in one chunk
    assert_eq!(chunks.len(), 1);
    assert!((chunks[0].start_time.unwrap() - 10.0).abs() < f32::EPSILON);
}

#[tokio::test]
async fn given_segments_exceed_token_budget_when_split_segments_called_then_multiple_chunks_produced()
 {
    // Very small budget to force splits
    let splitter = SemanticSplitter::new(10, 0).unwrap();
    let doc_id = DocumentId::new();
    let segments = vec![
        TranscriptSegment::new("This is the first long segment with many words.", 0.0, 5.0),
        TranscriptSegment::new(
            "This is the second long segment with many words.",
            5.5,
            10.0,
        ),
        TranscriptSegment::new(
            "This is the third long segment with many words.",
            10.5,
            15.0,
        ),
    ];

    let chunks = splitter
        .split_segments(&segments, doc_id, None)
        .await
        .unwrap();

    assert!(chunks.len() >= 2, "Expected at least 2 chunks");
    // First chunk must start at t=0
    assert!((chunks[0].start_time.unwrap() - 0.0).abs() < f32::EPSILON);
}

#[tokio::test]
async fn given_segments_with_metadata_when_split_segments_called_then_metadata_attached_to_chunks()
{
    let splitter = SemanticSplitter::new(512, 0).unwrap();
    let doc_id = DocumentId::new();
    let meta = Arc::new(DocumentMetadata {
        title: "CS101 Lecture".to_string(),
        content_type: ContentType::Video,
        source_url: Some("https://example.com/lecture.mp4".to_string()),
    });
    let segments = vec![TranscriptSegment::new(
        "Topic: sorting algorithms.",
        60.0,
        63.0,
    )];

    let chunks = splitter
        .split_segments(&segments, doc_id, Some(Arc::clone(&meta)))
        .await
        .unwrap();

    assert_eq!(chunks.len(), 1);
    let chunk = &chunks[0];
    assert!(chunk.metadata.is_some());
    assert_eq!(chunk.metadata.as_ref().unwrap().title, "CS101 Lecture");
    assert!((chunk.start_time.unwrap() - 60.0).abs() < f32::EPSILON);
}
