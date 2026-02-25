use sandakan::domain::{ToolCall, ToolCallId, ToolName, ToolResult};

#[test]
fn given_string_id_when_creating_tool_call_id_then_round_trips_as_str() {
    let id = ToolCallId::new("call_abc123");
    assert_eq!(id.as_str(), "call_abc123");
}

#[test]
fn given_string_name_when_creating_tool_name_then_round_trips_as_str() {
    let name = ToolName::new("web_search");
    assert_eq!(name.as_str(), "web_search");
}

#[test]
fn given_tool_call_when_cloning_then_produces_independent_copy() {
    let original = ToolCall {
        id: ToolCallId::new("call_001"),
        name: ToolName::new("web_search"),
        arguments: serde_json::json!({"query": "test"}),
    };
    let cloned = original.clone();

    assert_eq!(original.id.as_str(), cloned.id.as_str());
    assert_eq!(original.name.as_str(), cloned.name.as_str());
}

#[test]
fn given_tool_result_when_inspecting_fields_then_returns_expected_values() {
    let result = ToolResult {
        tool_call_id: ToolCallId::new("call_001"),
        tool_name: ToolName::new("web_search"),
        content: "Search results here".to_string(),
    };

    assert_eq!(result.tool_call_id.as_str(), "call_001");
    assert_eq!(result.tool_name.as_str(), "web_search");
    assert_eq!(result.content, "Search results here");
}

#[test]
fn given_tool_call_id_when_displaying_then_shows_inner_string() {
    let id = ToolCallId::new("call_xyz");
    assert_eq!(format!("{}", id), "call_xyz");
}
