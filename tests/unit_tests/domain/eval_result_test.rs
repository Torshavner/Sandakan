use sandakan::domain::{EvalEventId, EvalResult, EvalResultId};
use uuid::Uuid;

#[test]
fn given_faithfulness_above_threshold_when_result_created_then_below_threshold_is_false() {
    let event_id = EvalEventId::from_uuid(Uuid::new_v4());
    let result = EvalResult::new(event_id, 0.9, None, None, None, None, 0.7);
    assert!(!result.below_threshold);
}

#[test]
fn given_faithfulness_below_threshold_when_result_created_then_below_threshold_is_true() {
    let event_id = EvalEventId::from_uuid(Uuid::new_v4());
    let result = EvalResult::new(event_id, 0.5, None, None, None, None, 0.7);
    assert!(result.below_threshold);
}

#[test]
fn given_faithfulness_equal_to_threshold_when_result_created_then_below_threshold_is_false() {
    let event_id = EvalEventId::from_uuid(Uuid::new_v4());
    let result = EvalResult::new(event_id, 0.7, None, None, None, None, 0.7);
    assert!(!result.below_threshold);
}

#[test]
fn given_eval_result_when_serialized_to_json_then_round_trips_correctly() {
    let event_id = EvalEventId::from_uuid(Uuid::new_v4());
    let result = EvalResult::new(
        event_id,
        0.85,
        Some(0.9),
        Some(0.8),
        Some(0.6),
        Some(0.75),
        0.7,
    );
    let json = serde_json::to_string(&result).expect("serialization failed");
    let decoded: EvalResult = serde_json::from_str(&json).expect("deserialization failed");
    assert_eq!(decoded.eval_event_id.as_uuid(), event_id.as_uuid());
    assert!((decoded.faithfulness - 0.85_f32).abs() < f32::EPSILON);
    assert_eq!(decoded.answer_relevancy, Some(0.9_f32));
    assert_eq!(decoded.context_precision, Some(0.8_f32));
    assert_eq!(decoded.context_recall, Some(0.6_f32));
    assert_eq!(decoded.correctness, Some(0.75_f32));
    assert!(!decoded.below_threshold);
}

#[test]
fn given_eval_result_without_optional_metrics_when_serialized_then_optional_fields_are_null() {
    let event_id = EvalEventId::from_uuid(Uuid::new_v4());
    let result = EvalResult::new(event_id, 0.5, None, None, None, None, 0.7);
    let json = serde_json::to_string(&result).expect("serialization failed");
    let value: serde_json::Value = serde_json::from_str(&json).expect("parse failed");
    assert!(value["answer_relevancy"].is_null());
    assert!(value["context_precision"].is_null());
    assert!(value["context_recall"].is_null());
    assert!(value["correctness"].is_null());
}

#[test]
fn given_eval_result_id_when_round_tripped_through_uuid_then_preserves_value() {
    let uuid = Uuid::new_v4();
    let id = EvalResultId::from_uuid(uuid);
    assert_eq!(id.as_uuid(), uuid);
    assert_eq!(id.to_string(), uuid.to_string());
}
