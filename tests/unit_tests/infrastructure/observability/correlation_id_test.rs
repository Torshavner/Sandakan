use sandakan::infrastructure::observability::{CORRELATION_ID_HEADER, CorrelationId};

#[test]
fn given_correlation_id_header_constant_when_accessed_then_returns_correct_value() {
    assert_eq!(CORRELATION_ID_HEADER, "x-correlation-id");
}

#[test]
fn given_correlation_id_when_created_then_contains_value() {
    let id = CorrelationId("trace-abc-123".to_string());
    assert_eq!(id.0, "trace-abc-123");
}

#[test]
fn given_correlation_id_when_cloned_then_equals_original() {
    let original = CorrelationId("trace-xyz".to_string());
    let cloned = original.clone();
    assert_eq!(original.0, cloned.0);
}
