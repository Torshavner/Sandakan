use sandakan::application::services::count_tokens;

#[test]
fn given_empty_string_when_counting_then_returns_zero() {
    let result = count_tokens("");
    assert_eq!(result, 0);
}

#[test]
fn given_known_sentence_when_counting_then_returns_expected_count() {
    let result = count_tokens("Hello, world!");
    assert!(result > 0);
    assert!(result < 10);
}
