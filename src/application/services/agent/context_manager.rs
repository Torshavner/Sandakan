use crate::application::ports::{AgentMessage, LlmClient};
use crate::application::services::count_tokens;

fn estimate_message_tokens(msg: &AgentMessage) -> usize {
    match msg {
        AgentMessage::System(s) | AgentMessage::User(s) => count_tokens(s),
        AgentMessage::Assistant {
            content,
            tool_calls,
        } => {
            let c = content.as_deref().map(count_tokens).unwrap_or(0);
            let t: usize = tool_calls
                .iter()
                .map(|tc| {
                    let args =
                        serde_json::to_string(&tc.arguments).unwrap_or_else(|_| String::new());
                    count_tokens(tc.name.as_str()) + count_tokens(&args)
                })
                .sum();
            c + t
        }
        AgentMessage::ToolResult(r) => count_tokens(&r.content),
    }
}

fn is_prunable(msg: &AgentMessage) -> bool {
    matches!(
        msg,
        AgentMessage::Assistant { content: None, .. } | AgentMessage::ToolResult(_)
    )
}

pub(crate) fn auto_prune_if_needed(messages: &mut Vec<AgentMessage>, max_tokens: usize) -> usize {
    let total: usize = messages.iter().map(estimate_message_tokens).sum();
    if total <= max_tokens {
        return 0;
    }

    // Indices of prunable messages, oldest first.
    let prunable_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| is_prunable(m))
        .map(|(i, _)| i)
        .collect();

    // Keep the 2 most recent prunable messages (last tool exchange) intact.
    let safe_count = 2;
    let removable = if prunable_indices.len() > safe_count {
        prunable_indices.len() - safe_count
    } else {
        return 0;
    };
    let mut to_remove = Vec::with_capacity(removable);
    let mut running_total = total;

    for &idx in prunable_indices.iter().take(removable) {
        to_remove.push(idx);
        running_total = running_total.saturating_sub(estimate_message_tokens(&messages[idx]));
        if running_total <= max_tokens {
            break;
        }
    }

    let removed = to_remove.len();
    // Remove in reverse order so indices stay valid.
    for &idx in to_remove.iter().rev() {
        messages.remove(idx);
    }

    removed
}

/// Falls back to oldest-first pruning on any LLM failure to avoid blocking the agent.
pub(crate) async fn smart_prune_if_needed(
    messages: &mut Vec<AgentMessage>,
    max_tokens: usize,
    llm_client: &dyn LlmClient,
) -> usize {
    let total: usize = messages.iter().map(estimate_message_tokens).sum();
    if total <= max_tokens {
        return 0;
    }

    let prunable: Vec<(usize, String)> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| is_prunable(m))
        .map(|(idx, m)| {
            let preview = match m {
                AgentMessage::ToolResult(r) => {
                    let name = r.tool_name.as_str();
                    let content: String = r.content.chars().take(120).collect();
                    format!("[{idx}] tool={name} content=\"{content}\"")
                }
                AgentMessage::Assistant { tool_calls, .. } => {
                    let names: Vec<_> = tool_calls.iter().map(|c| c.name.as_str()).collect();
                    format!("[{idx}] tool_call={}", names.join(","))
                }
                _ => unreachable!(),
            };
            (idx, preview)
        })
        .collect();

    // Keep the 2 most recent prunable messages (last tool exchange) intact.
    let safe_count = 2usize;
    if prunable.len() <= safe_count {
        return 0;
    }
    let candidates = &prunable[..prunable.len() - safe_count];

    let user_question = messages
        .iter()
        .find_map(|m| match m {
            AgentMessage::User(t) => Some(t.as_str()),
            _ => None,
        })
        .unwrap_or("(unknown)");

    let summaries: Vec<&str> = candidates.iter().map(|(_, s)| s.as_str()).collect();
    let prompt = format!(
        "You are managing an AI agent's context window. \
         The user question is: \"{user_question}\"\n\n\
         The following tool-call/result messages are candidates for removal \
         to free context space. Score each entry 1 (safe to drop) to 5 (must keep) \
         based on how relevant it still is to answering the user question.\n\n\
         Respond ONLY with lines in the format: INDEX SCORE\n\
         where INDEX is the bracketed number and SCORE is 1-5.\n\n\
         Candidates:\n{}",
        summaries.join("\n")
    );

    let raw = match llm_client.complete(&prompt, "").await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "Smart prune: LLM call failed, falling back to oldest-first");
            return auto_prune_if_needed(messages, max_tokens);
        }
    };

    let mut scores: std::collections::HashMap<usize, u8> =
        candidates.iter().map(|(idx, _)| (*idx, 3u8)).collect();

    for line in raw.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let (Ok(idx), Ok(score)) = (parts[0].parse::<usize>(), parts[1].parse::<u8>()) {
                if scores.contains_key(&idx) {
                    scores.insert(idx, score.min(5));
                }
            }
        }
    }

    let mut ordered: Vec<usize> = candidates.iter().map(|(idx, _)| *idx).collect();
    ordered.sort_by_key(|idx| scores.get(idx).copied().unwrap_or(3));

    let mut to_remove: Vec<usize> = Vec::with_capacity(candidates.len());
    let mut running_total = total;
    for idx in ordered {
        to_remove.push(idx);
        running_total = running_total.saturating_sub(estimate_message_tokens(&messages[idx]));
        if running_total <= max_tokens {
            break;
        }
    }

    let removed = to_remove.len();
    to_remove.sort_unstable_by(|a, b| b.cmp(a)); // reverse order to keep indices valid
    for idx in to_remove {
        messages.remove(idx);
    }

    tracing::debug!(
        removed,
        "Smart prune: removed lowest-relevance tool messages"
    );
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ToolCall, ToolCallId, ToolName, ToolResult};

    fn system(text: &str) -> AgentMessage {
        AgentMessage::System(text.to_string())
    }

    fn user(text: &str) -> AgentMessage {
        AgentMessage::User(text.to_string())
    }

    fn tool_call_msg(name: &str) -> AgentMessage {
        AgentMessage::Assistant {
            content: None,
            tool_calls: vec![ToolCall {
                id: ToolCallId::new("call_1"),
                name: ToolName::new(name),
                arguments: serde_json::json!({"q": "test"}),
            }],
        }
    }

    fn tool_result_msg(content: &str) -> AgentMessage {
        AgentMessage::ToolResult(ToolResult {
            tool_call_id: ToolCallId::new("call_1"),
            tool_name: ToolName::new("search"),
            content: content.to_string(),
        })
    }

    #[test]
    fn given_messages_under_budget_when_auto_prune_then_nothing_removed() {
        let mut msgs = vec![system("hello"), user("world")];
        let removed = auto_prune_if_needed(&mut msgs, 100_000);
        assert_eq!(removed, 0);
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn given_messages_over_budget_when_auto_prune_then_oldest_tool_pairs_removed() {
        let big = "x ".repeat(5000); // ~5000 tokens
        let mut msgs = vec![
            system("sys"),
            user("q"),
            tool_call_msg("search"),
            tool_result_msg(&big),
            tool_call_msg("read"),
            tool_result_msg(&big),
            tool_call_msg("read2"),
            tool_result_msg("small result"),
        ];
        // Budget smaller than total but enough for system + user + last pair
        let removed = auto_prune_if_needed(&mut msgs, 6_000);
        assert!(removed > 0);
        // System and user always kept
        assert!(msgs.iter().any(|m| matches!(m, AgentMessage::System(_))));
        assert!(msgs.iter().any(|m| matches!(m, AgentMessage::User(_))));
    }

    #[test]
    fn given_messages_over_budget_when_auto_prune_then_system_and_user_preserved() {
        let big = "word ".repeat(10000);
        let mut msgs = vec![
            system("system prompt"),
            user("user question"),
            tool_call_msg("s"),
            tool_result_msg(&big),
            tool_call_msg("s2"),
            tool_result_msg("last"),
        ];
        auto_prune_if_needed(&mut msgs, 100);
        // System and user must survive
        assert!(matches!(&msgs[0], AgentMessage::System(s) if s == "system prompt"));
        assert!(matches!(&msgs[1], AgentMessage::User(s) if s == "user question"));
    }

    #[test]
    fn given_messages_over_budget_when_auto_prune_then_recent_tool_exchange_kept() {
        let big = "tok ".repeat(5000);
        let mut msgs = vec![
            system("s"),
            tool_call_msg("old_search"),
            tool_result_msg(&big),
            tool_call_msg("new_search"),
            tool_result_msg("recent result"),
        ];
        auto_prune_if_needed(&mut msgs, 500);
        // Last 2 prunable messages (tool_call + tool_result) must survive
        let prunable: Vec<_> = msgs.iter().filter(|m| is_prunable(m)).collect();
        assert!(prunable.len() >= 2);
        // The recent result should still be present
        assert!(msgs.iter().any(|m| matches!(
            m,
            AgentMessage::ToolResult(r) if r.content == "recent result"
        )));
    }

    #[test]
    fn given_no_prunable_messages_when_over_budget_then_nothing_removed() {
        let big = "word ".repeat(20000);
        let mut msgs = vec![system(&big), user(&big)];
        let original_len = msgs.len();
        let removed = auto_prune_if_needed(&mut msgs, 100);
        assert_eq!(removed, 0);
        assert_eq!(msgs.len(), original_len);
    }
}
