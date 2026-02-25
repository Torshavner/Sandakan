use sandakan::application::ports::EvalEventRepository;
use sandakan::domain::{EvalEvent, EvalSource};
use sandakan::infrastructure::persistence::JsonlEvalEventRepository;
use tempfile::NamedTempFile;

fn make_event(question: &str) -> EvalEvent {
    EvalEvent::new(
        question,
        "Generated answer",
        vec![EvalSource {
            text: "Chunk text".to_string(),
            page: Some(1),
            score: 0.9,
        }],
        "lmstudio/llama3",
        None,
    )
}

#[tokio::test]
async fn given_eval_event_when_recorded_then_appears_in_list() {
    let file = NamedTempFile::new().unwrap();
    let repo = JsonlEvalEventRepository::new(file.path());
    let event = make_event("What is RAG?");

    repo.record(&event).await.unwrap();

    let events = repo.list(None).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].question, "What is RAG?");
    assert_eq!(events[0].model_config, "lmstudio/llama3");
}

#[tokio::test]
async fn given_multiple_events_when_listing_with_limit_then_returns_requested_count() {
    let file = NamedTempFile::new().unwrap();
    let repo = JsonlEvalEventRepository::new(file.path());

    for i in 0..5 {
        repo.record(&make_event(&format!("Question {}", i)))
            .await
            .unwrap();
    }

    let events = repo.list(Some(3)).await.unwrap();
    assert_eq!(events.len(), 3);
}

#[tokio::test]
async fn given_multiple_events_when_sampling_then_returns_requested_count() {
    let file = NamedTempFile::new().unwrap();
    let repo = JsonlEvalEventRepository::new(file.path());

    for i in 0..10 {
        repo.record(&make_event(&format!("Question {}", i)))
            .await
            .unwrap();
    }

    let events = repo.sample(4).await.unwrap();
    assert_eq!(events.len(), 4);
}

#[tokio::test]
async fn given_sample_size_larger_than_events_when_sampling_then_returns_all() {
    let file = NamedTempFile::new().unwrap();
    let repo = JsonlEvalEventRepository::new(file.path());

    repo.record(&make_event("Only question")).await.unwrap();

    let events = repo.sample(100).await.unwrap();
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn given_nonexistent_file_when_listing_then_returns_empty_vec() {
    let repo = JsonlEvalEventRepository::new("/tmp/nonexistent_sandakan_eval.jsonl");
    let events = repo.list(None).await.unwrap();
    assert!(events.is_empty());
}

#[tokio::test]
async fn given_eval_event_when_serialized_and_recorded_then_all_fields_preserved() {
    let file = NamedTempFile::new().unwrap();
    let repo = JsonlEvalEventRepository::new(file.path());

    let event = EvalEvent::new(
        "Complex question?",
        "Detailed answer.",
        vec![
            EvalSource {
                text: "Source 1".to_string(),
                page: Some(3),
                score: 0.85,
            },
            EvalSource {
                text: "Source 2".to_string(),
                page: None,
                score: 0.72,
            },
        ],
        "azure/gpt-4",
        None,
    );

    repo.record(&event).await.unwrap();
    let events = repo.list(None).await.unwrap();

    assert_eq!(events[0].question, "Complex question?");
    assert_eq!(events[0].generated_answer, "Detailed answer.");
    assert_eq!(events[0].retrieved_sources.len(), 2);
    assert_eq!(events[0].retrieved_sources[0].page, Some(3));
    assert_eq!(events[0].retrieved_sources[1].page, None);
    assert_eq!(events[0].model_config, "azure/gpt-4");
}
