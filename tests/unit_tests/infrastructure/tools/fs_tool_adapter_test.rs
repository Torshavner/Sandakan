use sandakan::infrastructure::mcp::ToolHandler;
use sandakan::infrastructure::tools::{ListDirectoryTool, ReadFileTool, build_fs_tools};
use serde_json::json;
use tempfile::TempDir;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_temp_tree() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(dir.path().join("subdir")).unwrap();
    std::fs::write(dir.path().join("hello.txt"), b"Hello, world!").unwrap();
    std::fs::write(dir.path().join("subdir").join("nested.txt"), b"nested").unwrap();
    dir
}

fn make_tools(dir: &TempDir) -> (ListDirectoryTool, ReadFileTool) {
    let (list, read, _search) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200)
        .expect("build_fs_tools should succeed for a valid directory");
    (list, read)
}

// ─── list_directory tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn given_valid_relative_path_when_listing_directory_then_returns_formatted_entries() {
    let dir = make_temp_tree();
    let (list_tool, _) = make_tools(&dir);

    let result = list_tool.execute(&json!({ "path": "." })).await.unwrap();

    assert!(result.contains("[DIR]  subdir") || result.contains("[DIR] subdir"));
    assert!(result.contains("[FILE] hello.txt"));
}

#[tokio::test]
async fn given_root_path_dot_when_listing_then_lists_root_directory_entries() {
    let dir = make_temp_tree();
    let (list_tool, _) = make_tools(&dir);

    let result = list_tool.execute(&json!({ "path": "." })).await.unwrap();

    assert!(result.starts_with("Directory:"));
    assert!(result.contains("hello.txt"));
}

#[tokio::test]
async fn given_nested_subdirectory_when_listing_then_returns_nested_file_entries() {
    let dir = make_temp_tree();
    let (list_tool, _) = make_tools(&dir);

    let result = list_tool
        .execute(&json!({ "path": "subdir" }))
        .await
        .unwrap();

    assert!(result.contains("[FILE] nested.txt"));
}

#[tokio::test]
async fn given_nonexistent_path_when_listing_directory_then_returns_execution_failed() {
    use sandakan::application::ports::McpError;

    let dir = make_temp_tree();
    let (list_tool, _) = make_tools(&dir);

    let result = list_tool
        .execute(&json!({ "path": "does_not_exist" }))
        .await;

    assert!(matches!(result, Err(McpError::ExecutionFailed(_))));
}

#[tokio::test]
async fn given_max_entries_limit_when_listing_large_directory_then_truncates_with_notice() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..5 {
        std::fs::write(dir.path().join(format!("file{i}.txt")), b"x").unwrap();
    }
    let (list_tool, _, _) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 3).unwrap();

    let result = list_tool.execute(&json!({ "path": "." })).await.unwrap();

    assert!(result.contains("truncated at 3 entries"));
}

// ─── read_file tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn given_valid_text_file_when_reading_then_returns_file_contents_unchanged() {
    let dir = make_temp_tree();
    let (_, read_tool) = make_tools(&dir);

    let result = read_tool
        .execute(&json!({ "path": "hello.txt" }))
        .await
        .unwrap();

    assert_eq!(result, "Hello, world!");
}

#[tokio::test]
async fn given_large_file_when_reading_then_content_is_truncated_with_notice() {
    let dir = tempfile::tempdir().unwrap();
    let content = "A".repeat(100);
    std::fs::write(dir.path().join("big.txt"), content.as_bytes()).unwrap();

    let (_, read_tool, _) = build_fs_tools(dir.path().to_str().unwrap(), 10, 200).unwrap();

    let result = read_tool
        .execute(&json!({ "path": "big.txt" }))
        .await
        .unwrap();

    assert!(result.contains("[truncated"));
    assert!(result.contains("100 bytes total"));
    assert!(result.contains("showing first 10"));
    assert!(result.starts_with("AAAAAAAAAA"));
}

#[tokio::test]
async fn given_nonexistent_path_when_reading_file_then_returns_execution_failed() {
    use sandakan::application::ports::McpError;

    let dir = make_temp_tree();
    let (_, read_tool) = make_tools(&dir);

    let result = read_tool.execute(&json!({ "path": "ghost.txt" })).await;

    assert!(matches!(result, Err(McpError::ExecutionFailed(_))));
}

#[tokio::test]
async fn given_binary_file_when_reading_then_returns_binary_file_error() {
    use sandakan::application::ports::McpError;

    let dir = tempfile::tempdir().unwrap();
    let binary_data: Vec<u8> = vec![0x00, 0xFF, 0xFE, 0x80, 0x81, 0x82];
    std::fs::write(dir.path().join("data.bin"), &binary_data).unwrap();

    let (_, read_tool, _) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = read_tool.execute(&json!({ "path": "data.bin" })).await;

    assert!(matches!(result, Err(McpError::ExecutionFailed(ref msg)) if msg == "binary file"));
}

// ─── Path traversal tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn given_path_traversal_attempt_when_reading_file_then_returns_execution_failed() {
    use sandakan::application::ports::McpError;

    // Create a temp dir with a parent file to try to escape to
    let parent = tempfile::tempdir().unwrap();
    std::fs::write(parent.path().join("secret.txt"), b"top secret").unwrap();
    let child = tempfile::Builder::new()
        .prefix("child")
        .tempdir_in(parent.path())
        .unwrap();

    let (_, read_tool, _) = build_fs_tools(child.path().to_str().unwrap(), 32_768, 200).unwrap();

    // Attempt to escape to the parent directory's secret file
    let result = read_tool.execute(&json!({ "path": "../secret.txt" })).await;

    assert!(matches!(result, Err(McpError::ExecutionFailed(_))));
    if let Err(McpError::ExecutionFailed(msg)) = result {
        // Either "path not found" (canonicalize failed) or "path escapes root boundary"
        assert!(
            msg.contains("path escapes root boundary") || msg.contains("path not found"),
            "unexpected error: {msg}"
        );
    }
}

#[tokio::test]
async fn given_path_traversal_attempt_when_listing_directory_then_returns_execution_failed() {
    use sandakan::application::ports::McpError;

    let parent = tempfile::tempdir().unwrap();
    let child = tempfile::Builder::new()
        .prefix("child")
        .tempdir_in(parent.path())
        .unwrap();

    let (list_tool, _, _) = build_fs_tools(child.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = list_tool.execute(&json!({ "path": ".." })).await;

    assert!(matches!(result, Err(McpError::ExecutionFailed(_))));
    if let Err(McpError::ExecutionFailed(msg)) = result {
        assert!(
            msg.contains("path escapes root boundary") || msg.contains("path not found"),
            "unexpected error: {msg}"
        );
    }
}

// ─── Schema and tool_name tests ───────────────────────────────────────────────

#[test]
fn given_list_directory_schema_when_inspected_then_has_required_path_parameter() {
    let schema = ListDirectoryTool::tool_schema();
    assert_eq!(schema.name, "list_directory");
    assert!(schema.parameters["properties"]["path"].is_object());
    let required = schema.parameters["required"]
        .as_array()
        .expect("required must be an array");
    assert!(required.iter().any(|v| v.as_str() == Some("path")));
}

#[test]
fn given_read_file_schema_when_inspected_then_has_required_path_parameter() {
    let schema = ReadFileTool::tool_schema();
    assert_eq!(schema.name, "read_file");
    assert!(schema.parameters["properties"]["path"].is_object());
    let required = schema.parameters["required"]
        .as_array()
        .expect("required must be an array");
    assert!(required.iter().any(|v| v.as_str() == Some("path")));
}

#[tokio::test]
async fn given_list_directory_tool_when_querying_tool_name_then_returns_list_directory() {
    let dir = make_temp_tree();
    let (list_tool, _) = make_tools(&dir);
    assert_eq!(list_tool.tool_name(), "list_directory");
}

#[tokio::test]
async fn given_read_file_tool_when_querying_tool_name_then_returns_read_file() {
    let dir = make_temp_tree();
    let (_, read_tool) = make_tools(&dir);
    assert_eq!(read_tool.tool_name(), "read_file");
}

// ─── Construction error test ──────────────────────────────────────────────────

#[test]
fn given_nonexistent_root_path_when_building_fs_tools_then_returns_mcp_error() {
    use sandakan::application::ports::McpError;

    let result = build_fs_tools("/nonexistent/path/that/does/not/exist", 32_768, 200);
    assert!(matches!(result, Err(McpError::ExecutionFailed(_))));
}
