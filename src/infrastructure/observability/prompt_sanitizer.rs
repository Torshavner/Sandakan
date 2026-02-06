const MAX_VISIBLE_LENGTH: usize = 100;

/// Sanitizes prompt text for safe logging.
pub fn sanitize_prompt(prompt: &str) -> String {
    let trimmed = prompt.trim();

    if trimmed.is_empty() {
        return String::from("[EMPTY]");
    }

    let sanitized = if trimmed.len() > MAX_VISIBLE_LENGTH {
        format!(
            "{}... ({} chars total)",
            &trimmed[..MAX_VISIBLE_LENGTH],
            trimmed.len()
        )
    } else {
        trimmed.to_string()
    };

    redact_sensitive_patterns(&sanitized)
}

fn redact_sensitive_patterns(text: &str) -> String {
    let patterns = [
        ("Bearer ", "Bearer [REDACTED]"),
        ("api_key=", "api_key=[REDACTED]"),
        ("password=", "password=[REDACTED]"),
        ("secret=", "secret=[REDACTED]"),
        ("token=", "token=[REDACTED]"),
    ];

    let mut result = text.to_string();
    for (pattern, replacement) in patterns {
        if let Some(idx) = result.find(pattern) {
            let end = result[idx + pattern.len()..]
                .find(|c: char| c.is_whitespace() || c == '&' || c == '"' || c == '\'')
                .map(|i| idx + pattern.len() + i)
                .unwrap_or(result.len());
            result = format!("{}{}{}", &result[..idx], replacement, &result[end..]);
        }
    }

    result
}
