use sandakan::application::ports::RagSourceCollector;
use sandakan::domain::EvalSource;
use sandakan::infrastructure::tools::InMemoryRagSourceCollector;

fn make_source(text: &str, page: Option<u32>, score: f32) -> EvalSource {
    EvalSource {
        text: text.to_string(),
        page,
        score,
    }
}

#[test]
fn given_multiple_tool_calls_when_collecting_sources_then_drain_returns_all_accumulated() {
    let collector = InMemoryRagSourceCollector::new();

    collector.collect(vec![make_source("chunk A", Some(1), 0.9)]);
    collector.collect(vec![
        make_source("chunk B", Some(2), 0.8),
        make_source("chunk C", None, 0.7),
    ]);

    let drained = collector.drain();
    assert_eq!(drained.len(), 3);
    assert_eq!(drained[0].text, "chunk A");
    assert_eq!(drained[1].text, "chunk B");
    assert_eq!(drained[2].text, "chunk C");
}

#[test]
fn given_drain_called_when_no_sources_collected_then_returns_empty_vec() {
    let collector = InMemoryRagSourceCollector::new();
    let drained = collector.drain();
    assert!(drained.is_empty());
}

#[test]
fn given_drain_called_twice_when_sources_collected_once_then_second_drain_returns_empty_vec() {
    let collector = InMemoryRagSourceCollector::new();

    collector.collect(vec![make_source("only source", Some(5), 0.95)]);

    let first = collector.drain();
    assert_eq!(first.len(), 1);

    let second = collector.drain();
    assert!(
        second.is_empty(),
        "second drain should be empty after first consumed all sources"
    );
}
