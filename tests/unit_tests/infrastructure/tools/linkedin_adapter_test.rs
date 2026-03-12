use sandakan::application::ports::McpError;
use sandakan::infrastructure::mcp::ToolHandler;
use sandakan::infrastructure::tools::{LinkedInMimicAdapter, build_ugc_post};
use serde_json::json;

// ─── build_ugc_post helper ────────────────────────────────────────────────────

#[test]
fn given_valid_args_when_building_ugc_post_then_produces_correct_json_structure() {
    let post = build_ugc_post("urn:li:person:abc", "Hello LinkedIn!", "PUBLIC");

    assert_eq!(post["author"], "urn:li:person:abc");
    assert_eq!(post["lifecycleState"], "PUBLISHED");
    assert_eq!(
        post["specificContent"]["com.linkedin.ugc.ShareContent"]["shareCommentary"]["text"],
        "Hello LinkedIn!"
    );
    assert_eq!(
        post["specificContent"]["com.linkedin.ugc.ShareContent"]["shareMediaCategory"],
        "NONE"
    );
    assert!(post["visibility"]["com.linkedin.ugc.MemberNetworkVisibility"].is_string());
}

#[test]
fn given_public_visibility_when_building_ugc_post_then_sets_public_member_network_visibility() {
    let post = build_ugc_post("urn:li:person:abc", "Post body", "PUBLIC");

    assert_eq!(
        post["visibility"]["com.linkedin.ugc.MemberNetworkVisibility"],
        "PUBLIC"
    );
}

#[test]
fn given_connections_visibility_when_building_ugc_post_then_sets_connections_visibility() {
    let post = build_ugc_post("urn:li:person:abc", "Post body", "CONNECTIONS");

    assert_eq!(
        post["visibility"]["com.linkedin.ugc.MemberNetworkVisibility"],
        "CONNECTIONS"
    );
}

// ─── LinkedInMimicAdapter ─────────────────────────────────────────────────────

#[test]
fn given_mimic_adapter_when_querying_tool_name_then_returns_post_linkedin() {
    assert_eq!(LinkedInMimicAdapter.tool_name(), "post_linkedin");
}

#[tokio::test]
async fn given_missing_content_arg_when_executing_mimic_then_returns_serialization_error() {
    let result = LinkedInMimicAdapter.execute(&json!({})).await;

    assert!(matches!(result, Err(McpError::Serialization(_))));
    if let Err(McpError::Serialization(msg)) = result {
        assert!(msg.contains("missing 'content' argument"));
    }
}

#[tokio::test]
async fn given_valid_content_when_executing_mimic_then_returns_preview_string_with_content() {
    let args = json!({ "content": "Exciting news from our team!" });

    let result = LinkedInMimicAdapter.execute(&args).await.unwrap();

    assert!(result.contains("[linkedin_preview]"));
    assert!(result.contains("Exciting news from our team!"));
}

#[tokio::test]
async fn given_visibility_arg_when_executing_mimic_then_preview_includes_visibility() {
    let args = json!({ "content": "Team update", "visibility": "CONNECTIONS" });

    let result = LinkedInMimicAdapter.execute(&args).await.unwrap();

    assert!(result.contains("visibility=CONNECTIONS"));
}

// ─── LinkedInAdapter schema ───────────────────────────────────────────────────

#[test]
fn given_linkedin_tool_schema_when_inspected_then_content_is_required_and_visibility_is_optional() {
    use sandakan::infrastructure::tools::LinkedInAdapter;

    let schema = LinkedInAdapter::tool_schema();

    assert_eq!(schema.name, "post_linkedin");
    assert!(schema.parameters["properties"]["content"].is_object());
    assert!(schema.parameters["properties"]["visibility"].is_object());

    let required = schema.parameters["required"]
        .as_array()
        .expect("required must be an array");
    assert!(required.iter().any(|v| v.as_str() == Some("content")));
    // visibility is optional — must NOT appear in required
    assert!(!required.iter().any(|v| v.as_str() == Some("visibility")));
}
