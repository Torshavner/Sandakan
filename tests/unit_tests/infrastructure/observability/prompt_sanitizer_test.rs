use sandakan::infrastructure::observability::sanitize_prompt;

#[test]
fn given_empty_prompt_when_sanitizing_then_returns_empty_marker() {
    assert_eq!(sanitize_prompt(""), "[EMPTY]");
    assert_eq!(sanitize_prompt("   "), "[EMPTY]");
}

#[test]
fn given_short_prompt_when_sanitizing_then_returns_unchanged() {
    let prompt = "What is the weather today?";
    assert_eq!(sanitize_prompt(prompt), prompt);
}

#[test]
fn given_long_prompt_when_sanitizing_then_truncates_with_length() {
    let prompt = "a".repeat(150);
    let result = sanitize_prompt(&prompt);
    assert!(result.contains("... (150 chars total)"));
    assert!(result.starts_with(&"a".repeat(100)));
}

#[test]
fn given_bearer_token_when_sanitizing_then_redacts_token() {
    let prompt = "Authorization: Bearer sk-abc123xyz";
    let result = sanitize_prompt(prompt);
    assert!(result.contains("Bearer [REDACTED]"));
    assert!(!result.contains("sk-abc123xyz"));
}

#[test]
fn given_api_key_when_sanitizing_then_redacts_key() {
    let prompt = "Send request with api_key=secret123";
    let result = sanitize_prompt(prompt);
    assert!(result.contains("api_key=[REDACTED]"));
    assert!(!result.contains("secret123"));
}

#[test]
fn given_password_when_sanitizing_then_redacts_password() {
    let prompt = "Login with password=hunter2";
    let result = sanitize_prompt(prompt);
    assert!(result.contains("password=[REDACTED]"));
    assert!(!result.contains("hunter2"));
}

#[test]
fn given_whitespace_padded_prompt_when_sanitizing_then_trims() {
    let prompt = "  Hello world  ";
    assert_eq!(sanitize_prompt(prompt), "Hello world");
}
