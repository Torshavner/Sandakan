use sandakan::domain::TranscriptSegment;

#[test]
fn given_segment_text_and_times_when_created_then_fields_are_stored() {
    let seg = TranscriptSegment::new("Hello world", 0.5, 3.2);
    assert_eq!(seg.text, "Hello world");
    assert!((seg.start_time - 0.5).abs() < f32::EPSILON);
    assert!((seg.end_time - 3.2).abs() < f32::EPSILON);
}

#[test]
fn given_multiple_segments_when_merge_text_called_then_returns_space_joined_text() {
    let segments = vec![
        TranscriptSegment::new("First.", 0.0, 2.0),
        TranscriptSegment::new("Second.", 2.1, 4.0),
        TranscriptSegment::new("Third.", 4.2, 6.0),
    ];
    let merged = TranscriptSegment::merge_text(&segments);
    assert_eq!(merged, "First. Second. Third.");
}

#[test]
fn given_empty_segments_when_merge_text_called_then_returns_empty_string() {
    let merged = TranscriptSegment::merge_text(&[]);
    assert_eq!(merged, "");
}

#[test]
fn given_segments_with_whitespace_text_when_merge_text_called_then_blank_segments_are_skipped() {
    let segments = vec![
        TranscriptSegment::new("  ", 0.0, 1.0),
        TranscriptSegment::new("Real content.", 1.0, 3.0),
    ];
    let merged = TranscriptSegment::merge_text(&segments);
    assert_eq!(merged, "Real content.");
}
