use sandakan::domain::{EvalEntry, EvalEvent, EvalSource};

#[test]
fn given_valid_jsonl_line_when_parsed_then_entry_has_required_fields() {
    let line = r#"{"question":"What is chunking?","expected_answer":"Splitting text into pieces.","expected_source_pages":[1,2]}"#;
    let entry: EvalEntry = serde_json::from_str(line).unwrap();
    assert_eq!(entry.question, "What is chunking?");
    assert_eq!(entry.expected_answer, "Splitting text into pieces.");
    assert_eq!(entry.expected_source_pages, Some(vec![1, 2]));
}

#[test]
fn given_jsonl_missing_question_when_parsed_then_returns_error() {
    let line = r#"{"expected_answer":"Some answer."}"#;
    let result: Result<EvalEntry, _> = serde_json::from_str(line);
    assert!(result.is_err());
}

#[test]
fn given_jsonl_missing_expected_answer_when_parsed_then_returns_error() {
    let line = r#"{"question":"What is this?"}"#;
    let result: Result<EvalEntry, _> = serde_json::from_str(line);
    assert!(result.is_err());
}

#[test]
fn given_jsonl_without_source_pages_when_parsed_then_pages_is_none() {
    let line = r#"{"question":"What is this?","expected_answer":"This is something."}"#;
    let entry: EvalEntry = serde_json::from_str(line).unwrap();
    assert!(entry.expected_source_pages.is_none());
}

#[test]
fn given_jsonl_with_null_source_pages_when_parsed_then_pages_is_none() {
    let line = r#"{"question":"Q?","expected_answer":"A.","expected_source_pages":null}"#;
    let entry: EvalEntry = serde_json::from_str(line).unwrap();
    assert!(entry.expected_source_pages.is_none());
}

#[test]
fn given_eval_event_when_built_then_sources_match_input() {
    let sources = vec![EvalSource {
        text: "chunk content".to_string(),
        page: Some(5),
        score: 0.88,
    }];
    let event = EvalEvent::new("Test question?", "Test answer.", sources, "lmstudio/llama3");

    assert_eq!(event.question, "Test question?");
    assert_eq!(event.generated_answer, "Test answer.");
    assert_eq!(event.retrieved_sources.len(), 1);
    assert_eq!(event.retrieved_sources[0].page, Some(5));
    assert_eq!(event.model_config, "lmstudio/llama3");
}

#[test]
fn given_eval_event_with_multiple_sources_when_getting_context_text_then_joined_by_double_newline()
{
    let sources = vec![
        EvalSource {
            text: "First chunk".to_string(),
            page: Some(1),
            score: 0.9,
        },
        EvalSource {
            text: "Second chunk".to_string(),
            page: Some(2),
            score: 0.8,
        },
    ];
    let event = EvalEvent::new("Q?", "A.", sources, "model");
    assert_eq!(event.context_text(), "First chunk\n\nSecond chunk");
}
