use crate::application::ports::AgentMessage;

/// Returns `true` when every tool result in the batch is an error or timeout.
pub(crate) fn all_tool_results_failed(results: &[&crate::domain::ToolResult]) -> bool {
    !results.is_empty()
        && results.iter().all(|r| {
            r.content.starts_with("[tool_error]") || r.content.starts_with("[tool_timeout]")
        })
}

/// Builds a context-rich prompt for the critic by extracting the user question,
/// tool results, and candidate answer from the full message history.
pub(crate) fn build_critic_prompt(
    messages: &[AgentMessage],
    critic_system_prompt: &str,
    candidate_answer: &str,
) -> String {
    let user_question = messages
        .iter()
        .find_map(|m| match m {
            AgentMessage::User(text) => Some(text.as_str()),
            _ => None,
        })
        .unwrap_or("(unknown question)");

    let tool_context: Vec<String> = messages
        .iter()
        .filter_map(|m| match m {
            AgentMessage::ToolResult(r)
                if !r.content.starts_with("[tool_error]")
                    && !r.content.starts_with("[tool_timeout]") =>
            {
                Some(truncate_for_event(&r.content, 500))
            }
            _ => None,
        })
        .collect();

    let sources_section = if tool_context.is_empty() {
        "No tool results available.".to_string()
    } else {
        tool_context.join("\n---\n")
    };

    format!(
        "{critic_system_prompt}\n\n\
         User question:\n{user_question}\n\n\
         Retrieved context:\n{sources_section}\n\n\
         Candidate answer:\n{candidate_answer}"
    )
}

/// Truncates `s` to at most `max_bytes` bytes without splitting a UTF-8 codepoint.
///
/// `s.len()` is a byte count, so a naive `&s[..max_bytes]` panics when the cut
/// lands inside a multi-byte sequence (CJK, emoji, accented text). Walking
/// `char_indices` finds the last safe codepoint boundary at or before the limit.
pub(crate) fn truncate_for_event(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // Find the byte offset of the last char that fits entirely within max_bytes.
    let boundary = s
        .char_indices()
        .take_while(|(byte_pos, ch)| byte_pos + ch.len_utf8() <= max_bytes)
        .last()
        .map(|(byte_pos, ch)| byte_pos + ch.len_utf8())
        .unwrap_or(0);
    format!("{}…", &s[..boundary])
}

/// Parses `SCORE: 0.X` and `ISSUES: ...` lines from a critic response.
///
/// Returns `(1.0, [])` on any parse failure so the caller treats the answer as
/// passing and skips the correction pass (graceful degradation).
pub(crate) fn parse_critic_response(raw: &str) -> (f32, Vec<String>) {
    let mut score: f32 = 1.0;
    let mut issues: Vec<String> = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("SCORE:") {
            if let Ok(v) = rest.trim().parse::<f32>() {
                score = v.clamp(0.0, 1.0);
            }
        } else if let Some(rest) = trimmed.strip_prefix("ISSUES:") {
            let rest = rest.trim();
            if !rest.eq_ignore_ascii_case("none") {
                issues = rest
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }

    (score, issues)
}
