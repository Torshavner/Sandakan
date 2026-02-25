# Agent: LinkedIn Post Generation Tool (`generate_post`)

## Status: BACKLOG

## Requirement Definition

As a **capstone deliverable owner**, I need **the agent to generate a professional, authentic LinkedIn post about itself** so that **it can describe what it does and how it was built in its own words, referencing actual implementation details from the codebase, as required by the Ciklum AI Academy submission**.

---

## Context

The capstone optionally requires a *"short social-style message created by your agent itself"* (5–7 sentences, professional, mentioning Ciklum AI Academy). This story adds a `generate_post` `ToolHandler` that wraps a prompt-engineering workflow: the agent uses `read_file` (US-029) to gather context from key source files, then generates a structured post via the LLM with a fixed system prompt.

The tool is intentionally thin — it is a prompt-engineering wrapper, not a new LLM integration. It uses the existing `LlmClient` (already injected into `AgentService`) via a new `PostGeneratorAdapter` that holds a reference to `Arc<dyn LlmClient>`. The output is a ready-to-publish post in plain text.

**Design note:** This tool is not about sending the post to LinkedIn — that would require OAuth and is out of scope. It generates the text content so the user can publish it manually. Optionally the `send_notification` tool (US-025) could be chained to push the post text to a configured webhook.

---

## Architecture

```
L2 Ports:   (none new)
L3 Tools:   PostGeneratorAdapter implements ToolHandler
            ├─ holds Arc<dyn LlmClient> (injected at construction)
            └─ calls LlmClient::complete(post_system_prompt, context)
                ↑ registered via
L4 Main:    settings.agent.post_generator (optional)
```

The adapter uses `LlmClient::complete()` — not `complete_with_tools()` — because post generation is a single focused completion with no tool loop.

---

## Layer Responsibilities (Hexagonal, maintained)

| Layer | Change |
|---|---|
| **L1 Domain** | None |
| **L2 Ports** | None |
| **L3 Infrastructure** | New `src/infrastructure/tools/post_generator_adapter.rs` — `PostGeneratorAdapter` implementing `ToolHandler` |
| **L4 Presentation** | `PostGeneratorSettings` added to `AgentSettings`; conditional wiring in `main.rs` |

---

## Implementation Notes

### `PostGeneratorAdapter` (L3)

```rust
pub struct PostGeneratorSettings {
    #[serde(default = "default_post_system_prompt")]
    pub system_prompt: String,
    #[serde(default = "default_post_platform")]
    pub platform: String,  // "linkedin" | "twitter" | "generic"
}

pub struct PostGeneratorAdapter {
    llm_client: Arc<dyn LlmClient>,
    config: PostGeneratorSettings,
}
```

- `tool_name()` → `"generate_post"`
- `tool_schema()`:
  ```json
  {
    "name": "generate_post",
    "description": "Generate a professional social media post about this AI agent system. Provide context about what the system does, how it was built, and any highlights you want included. The output is ready to publish.",
    "parameters": {
      "type": "object",
      "properties": {
        "context": {
          "type": "string",
          "description": "Key facts, implementation details, or highlights to include in the post. Gather this using read_file on key source files first."
        },
        "platform": {
          "type": "string",
          "enum": ["linkedin", "twitter", "generic"],
          "description": "Target platform — affects tone and length."
        }
      },
      "required": ["context"]
    }
  }
  ```

- `execute()`:
  1. Extract `arguments["context"].as_str()` → `McpError::Serialization` if absent.
  2. Extract `arguments["platform"].as_str().unwrap_or(&self.config.platform)`.
  3. Build prompt from `context` and platform-specific length hint.
  4. Call `self.llm_client.complete(&system_prompt, context).await`.
  5. Return generated post text as `Ok(post_text)`.

**Default system prompt:**
```
You are writing a professional social media post on behalf of an AI engineering project.

Guidelines:
- Write in first person as the AI system itself
- Length: 5–7 sentences for LinkedIn, 1–2 for Twitter
- Mention: what the system does, the technology used (Rust, RAG, agentic reasoning), and that it was built as part of the Ciklum AI Academy
- Tone: professional, authentic, concise — no buzzword soup
- End with a relevant hashtag set if LinkedIn
- Do NOT include any preamble — output only the post text

Context about the system:
```

### `AgentSettings` addition

```rust
#[serde(default)]
pub post_generator: Option<PostGeneratorSettings>,
```

### `main.rs` wiring

```rust
if let Some(pg_cfg) = &settings.agent.post_generator {
    let adapter = Arc::new(PostGeneratorAdapter::new(
        Arc::clone(&llm_client),
        pg_cfg.clone(),
    ));
    schemas.push(PostGeneratorAdapter::tool_schema());
    handlers.push(adapter as Arc<dyn ToolHandler>);
}
```

Note: `llm_client` is already constructed before the agent tool wiring block in `main.rs` — no new dependency is introduced.

---

## Typical Agent Workflow (using US-029 + US-031)

When a user prompts *"Generate a LinkedIn post about this system"*, the agent's ReAct loop would naturally:

1. Call `list_directory("src/")` → understand project structure.
2. Call `read_file("src/application/services/agent_service.rs")` → understand agentic loop.
3. Call `read_file(".ai/architecture.md")` → understand overall design.
4. Call `generate_post(context: "<summarised facts>", platform: "linkedin")` → produce post.
5. (Optionally) Call `send_notification(message: "<post>")` → push to webhook.

This chain grounds the post in actual code, making it authentic rather than hallucinated.

---

## File Checklist

| File | Action |
|---|---|
| `src/infrastructure/tools/post_generator_adapter.rs` | Create |
| `src/infrastructure/tools/mod.rs` | Modify — `mod` + `pub use` + `@AI` map entry |
| `src/presentation/config/settings.rs` | Modify — add `PostGeneratorSettings` to `AgentSettings` |
| `src/main.rs` | Modify — conditional wiring |
| `tests/unit_tests/infrastructure/tools/post_generator_adapter_test.rs` | Create |

---

## Acceptance Criteria

```gherkin
Scenario: Agent generates LinkedIn post from provided context
  Given agent.post_generator is configured
  When the LLM calls generate_post with context and platform = "linkedin"
  Then the tool calls LlmClient::complete with a LinkedIn-tailored system prompt
  And returns the generated post text as the tool result

Scenario: Platform defaults to config value when not specified in arguments
  Given agent.post_generator.platform = "linkedin"
  When generate_post is called without a platform argument
  Then the default platform "linkedin" is used

Scenario: Missing context argument returns serialization error
  When generate_post is called with no context argument
  Then McpError::Serialization is returned

Scenario: generate_post not registered when post_generator absent from config
  Given agent.post_generator is absent
  When the agent starts
  Then generate_post is not in the StaticToolRegistry

Scenario: LLM failure returns execution error
  Given LlmClient::complete returns an error
  When generate_post is called
  Then McpError::ExecutionFailed is returned with the LLM error message
```

---

## Test Strategy

Unit tests in `tests/unit_tests/infrastructure/tools/post_generator_adapter_test.rs`. Use `MockLlmClient` (already exists in `src/infrastructure/llm/mock_llm_client.rs`).

- `given_valid_context_when_generating_linkedin_post_then_returns_llm_completion`
- `given_missing_context_argument_when_executing_then_returns_serialization_error`
- `given_llm_client_fails_when_generating_post_then_returns_execution_failed`
- `given_platform_absent_in_args_when_executing_then_uses_config_default_platform`

## Dependencies

- **US-029** (Repository Introspection Tool) — the `generate_post` tool is most useful when the agent can first read source files to gather authentic context. Functional without US-029, but the generated post will be less grounded.
- **US-021** (agent infrastructure shipped) — agent service must exist.
- No dependency on US-030 (Self-Reflection) or US-025 (Notification), though they compose well with this tool.
