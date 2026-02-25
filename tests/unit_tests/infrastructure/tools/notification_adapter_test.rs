use sandakan::infrastructure::tools::{NotificationFormat, build_notification_body};
use serde_json::json;

// ─── Body construction tests (pure, no HTTP) ─────────────────────────────────

#[test]
fn given_plain_format_when_building_body_with_message_then_produces_text_only_payload() {
    let body = build_notification_body(&NotificationFormat::Plain, "", "Hello world");
    assert_eq!(body, json!({ "text": "Hello world" }));
}

#[test]
fn given_plain_format_when_building_body_with_title_then_title_is_ignored_in_payload() {
    let body = build_notification_body(&NotificationFormat::Plain, "My Title", "Done");
    assert_eq!(body, json!({ "text": "Done" }));
}

#[test]
fn given_slack_format_when_building_body_with_title_then_posts_bold_title_in_body() {
    let body = build_notification_body(&NotificationFormat::Slack, "Summary", "All tasks complete");
    let text = body["text"].as_str().expect("text field must be a string");
    assert_eq!(text, "*Summary*\nAll tasks complete");
}

#[test]
fn given_slack_format_when_title_absent_then_uses_empty_title_without_error() {
    let body = build_notification_body(&NotificationFormat::Slack, "", "Only message");
    let text = body["text"].as_str().expect("text field must be a string");
    assert_eq!(text, "Only message");
}

// ─── ToolHandler interface tests ─────────────────────────────────────────────

#[tokio::test]
async fn given_missing_message_argument_when_executing_then_returns_serialization_error() {
    use sandakan::application::ports::McpError;
    use sandakan::infrastructure::mcp::ToolHandler;
    use sandakan::infrastructure::tools::{NotificationAdapter, NotificationConfig};

    let adapter = NotificationAdapter::new(NotificationConfig {
        webhook_url: "http://localhost:9999/nonexistent".to_string(),
        format: NotificationFormat::Plain,
        timeout_secs: 1,
    })
    .expect("reqwest client construction must succeed in tests");

    let result = adapter.execute(&json!({})).await;

    assert!(matches!(result, Err(McpError::Serialization(_))));
    if let Err(McpError::Serialization(msg)) = result {
        assert!(msg.contains("missing 'message' argument"));
    }
}

#[test]
fn given_notification_adapter_when_querying_tool_name_then_returns_send_notification() {
    use sandakan::infrastructure::mcp::ToolHandler;
    use sandakan::infrastructure::tools::{NotificationAdapter, NotificationConfig};

    let adapter = NotificationAdapter::new(NotificationConfig {
        webhook_url: "http://localhost:9999/hook".to_string(),
        format: NotificationFormat::Plain,
        timeout_secs: 5,
    })
    .expect("reqwest client construction must succeed in tests");
    assert_eq!(adapter.tool_name(), "send_notification");
}

#[test]
fn given_notification_tool_schema_when_inspected_then_has_required_message_parameter() {
    use sandakan::infrastructure::tools::NotificationAdapter;

    let schema = NotificationAdapter::tool_schema();
    assert_eq!(schema.name, "send_notification");
    assert!(schema.parameters["properties"]["message"].is_object());
    let required = schema.parameters["required"]
        .as_array()
        .expect("required must be an array");
    assert!(required.iter().any(|v| v.as_str() == Some("message")));
    // title is optional — must NOT appear in required
    assert!(!required.iter().any(|v| v.as_str() == Some("title")));
}
