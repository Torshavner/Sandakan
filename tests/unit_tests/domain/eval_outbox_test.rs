use sandakan::domain::{EvalEventId, EvalOutboxEntry, EvalOutboxStatus};

#[test]
fn given_new_outbox_entry_when_constructed_then_status_is_pending() {
    let event_id = EvalEventId::new();
    let entry = EvalOutboxEntry::new(event_id);

    assert_eq!(entry.status, EvalOutboxStatus::Pending);
    assert_eq!(entry.eval_event_id, event_id);
    assert!(entry.error.is_none());
}

#[test]
fn given_outbox_status_when_round_tripped_through_string_then_preserves_value() {
    let statuses = [
        EvalOutboxStatus::Pending,
        EvalOutboxStatus::Processing,
        EvalOutboxStatus::Done,
        EvalOutboxStatus::Failed,
    ];

    for status in &statuses {
        let s = status.as_str();
        let parsed: EvalOutboxStatus = s.parse().expect("should parse");
        assert_eq!(&parsed, status);
    }
}

#[test]
fn given_invalid_string_when_parsing_outbox_status_then_returns_error() {
    let result = "invalid".parse::<EvalOutboxStatus>();
    assert!(result.is_err());
}

#[test]
fn given_outbox_entry_when_serialized_then_round_trips_through_json() {
    let entry = EvalOutboxEntry::new(EvalEventId::new());

    let json = serde_json::to_string(&entry).expect("should serialize");
    let deserialized: EvalOutboxEntry = serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(deserialized.id, entry.id);
    assert_eq!(deserialized.eval_event_id, entry.eval_event_id);
    assert_eq!(deserialized.status, entry.status);
}
