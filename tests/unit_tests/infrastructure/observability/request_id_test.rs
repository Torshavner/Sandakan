use sandakan::infrastructure::observability::{REQUEST_ID_HEADER, RequestId};

#[test]
fn given_request_id_header_constant_when_accessed_then_returns_correct_value() {
    assert_eq!(REQUEST_ID_HEADER, "x-request-id");
}

#[test]
fn given_request_id_when_created_then_contains_value() {
    let request_id = RequestId("test-123".to_string());
    assert_eq!(request_id.0, "test-123");
}

#[test]
fn given_request_id_when_cloned_then_equals_original() {
    let original = RequestId("abc".to_string());
    let cloned = original.clone();
    assert_eq!(original.0, cloned.0);
}
