use sandakan::infrastructure::mcp::ToolHandler;
use sandakan::infrastructure::tools::{
    GetFunctionSignaturesTool, ListDirectoryTool, ReadFileTool, SearchFilesTool, build_fs_tools,
};
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

fn make_tools(
    dir: &TempDir,
) -> (
    ListDirectoryTool,
    ReadFileTool,
    SearchFilesTool,
    GetFunctionSignaturesTool,
) {
    build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200)
        .expect("build_fs_tools should succeed for a valid directory")
}

// ─── list_directory tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn given_valid_relative_path_when_listing_directory_then_returns_formatted_entries() {
    let dir = make_temp_tree();
    let (list_tool, _, _, _) = make_tools(&dir);

    let result = list_tool.execute(&json!({ "path": "." })).await.unwrap();

    assert!(result.contains("[DIR]  subdir") || result.contains("[DIR] subdir"));
    assert!(result.contains("[FILE] hello.txt"));
}

#[tokio::test]
async fn given_root_path_dot_when_listing_then_lists_root_directory_entries() {
    let dir = make_temp_tree();
    let (list_tool, _, _, _) = make_tools(&dir);

    let result = list_tool.execute(&json!({ "path": "." })).await.unwrap();

    assert!(result.starts_with("Directory:"));
    assert!(result.contains("hello.txt"));
}

#[tokio::test]
async fn given_nested_subdirectory_when_listing_then_returns_nested_file_entries() {
    let dir = make_temp_tree();
    let (list_tool, _, _, _) = make_tools(&dir);

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
    let (list_tool, _, _, _) = make_tools(&dir);

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
    let (list_tool, _, _, _) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 3).unwrap();

    let result = list_tool.execute(&json!({ "path": "." })).await.unwrap();

    assert!(result.contains("truncated at 3 entries"));
}

// ─── read_file tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn given_valid_text_file_when_reading_then_returns_file_contents_unchanged() {
    let dir = make_temp_tree();
    let (_, read_tool, _, _) = make_tools(&dir);

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

    let (_, read_tool, _, _) = build_fs_tools(dir.path().to_str().unwrap(), 10, 200).unwrap();

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
    let (_, read_tool, _, _) = make_tools(&dir);

    let result = read_tool.execute(&json!({ "path": "ghost.txt" })).await;

    assert!(matches!(result, Err(McpError::ExecutionFailed(_))));
}

#[tokio::test]
async fn given_binary_file_when_reading_then_returns_binary_file_error() {
    use sandakan::application::ports::McpError;

    let dir = tempfile::tempdir().unwrap();
    let binary_data: Vec<u8> = vec![0x00, 0xFF, 0xFE, 0x80, 0x81, 0x82];
    std::fs::write(dir.path().join("data.bin"), &binary_data).unwrap();

    let (_, read_tool, _, _) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = read_tool.execute(&json!({ "path": "data.bin" })).await;

    assert!(matches!(result, Err(McpError::ExecutionFailed(ref msg)) if msg == "binary file"));
}

// ─── read_file line-range tests ───────────────────────────────────────────────

#[tokio::test]
async fn given_start_and_end_line_when_reading_file_then_returns_only_selected_lines() {
    let dir = tempfile::tempdir().unwrap();
    let content = "line1\nline2\nline3\nline4\nline5";
    std::fs::write(dir.path().join("lines.txt"), content.as_bytes()).unwrap();
    let (_, read_tool, _, _) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = read_tool
        .execute(&json!({ "path": "lines.txt", "start_line": 2, "end_line": 4 }))
        .await
        .unwrap();

    assert!(result.contains("2: line2"));
    assert!(result.contains("3: line3"));
    assert!(result.contains("4: line4"));
    assert!(!result.contains("line1"));
    assert!(!result.contains("line5"));
}

#[tokio::test]
async fn given_only_start_line_when_reading_file_then_returns_from_start_to_end_of_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), b"a\nb\nc\nd").unwrap();
    let (_, read_tool, _, _) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = read_tool
        .execute(&json!({ "path": "f.txt", "start_line": 3 }))
        .await
        .unwrap();

    assert!(result.contains("3: c"));
    assert!(result.contains("4: d"));
    assert!(!result.contains("1: a"));
}

#[tokio::test]
async fn given_start_line_beyond_file_length_when_reading_then_returns_execution_failed() {
    use sandakan::application::ports::McpError;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("short.txt"), b"only one line").unwrap();
    let (_, read_tool, _, _) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = read_tool
        .execute(&json!({ "path": "short.txt", "start_line": 99 }))
        .await;

    assert!(matches!(result, Err(McpError::ExecutionFailed(_))));
}

// ─── search_files context_lines tests ────────────────────────────────────────

#[tokio::test]
async fn given_context_lines_when_searching_files_then_returns_surrounding_lines() {
    let dir = tempfile::tempdir().unwrap();
    let content = "alpha\nbeta\ngamma\ndelta\nepsilon";
    std::fs::write(dir.path().join("ctx.txt"), content.as_bytes()).unwrap();
    let (_, _, search_tool, _) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = search_tool
        .execute(&json!({ "pattern": "gamma", "context_lines": 1 }))
        .await
        .unwrap();

    assert!(result.contains("beta"));
    assert!(result.contains("gamma"));
    assert!(result.contains("delta"));
}

#[tokio::test]
async fn given_no_context_lines_when_searching_files_then_returns_only_matching_lines() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("exact.txt"), b"foo\nbar\nbaz").unwrap();
    let (_, _, search_tool, _) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = search_tool
        .execute(&json!({ "pattern": "bar" }))
        .await
        .unwrap();

    assert!(result.contains("bar"));
    assert!(!result.contains("foo"));
    assert!(!result.contains("baz"));
}

// ─── get_function_signatures tests ───────────────────────────────────────────

#[tokio::test]
async fn given_rust_source_file_when_getting_signatures_then_returns_fn_declarations() {
    let dir = tempfile::tempdir().unwrap();
    let src = b"pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\nfn private() {}\n";
    std::fs::write(dir.path().join("lib.rs"), src).unwrap();
    let (_, _, _, sig_tool) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = sig_tool
        .execute(&json!({ "path": "lib.rs" }))
        .await
        .unwrap();

    assert!(result.contains("pub fn add"));
    assert!(result.contains("fn private"));
    assert!(!result.contains("a + b"));
}

#[tokio::test]
async fn given_python_source_file_when_getting_signatures_then_returns_def_declarations() {
    let dir = tempfile::tempdir().unwrap();
    let src = b"def greet(name: str) -> str:\n    return f'Hello {name}'\n\nasync def fetch():\n    pass\n";
    std::fs::write(dir.path().join("hello.py"), src).unwrap();
    let (_, _, _, sig_tool) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = sig_tool
        .execute(&json!({ "path": "hello.py" }))
        .await
        .unwrap();

    assert!(result.contains("def greet"));
    assert!(result.contains("async def fetch"));
    assert!(!result.contains("return"));
}

#[tokio::test]
async fn given_file_with_no_functions_when_getting_signatures_then_returns_none_found_message() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("data.txt"),
        b"just some text\nno functions here\n",
    )
    .unwrap();
    let (_, _, _, sig_tool) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = sig_tool
        .execute(&json!({ "path": "data.txt" }))
        .await
        .unwrap();

    assert!(result.contains("No code definitions found"));
}

#[tokio::test]
async fn given_get_function_signatures_schema_when_inspected_then_has_required_path_parameter() {
    let schema = GetFunctionSignaturesTool::tool_schema();
    assert_eq!(schema.name, "get_function_signatures");
    let required = schema.parameters["required"]
        .as_array()
        .expect("required must be an array");
    assert!(required.iter().any(|v| v.as_str() == Some("path")));
}

// ─── Path traversal tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn given_path_traversal_attempt_when_reading_file_then_returns_execution_failed() {
    use sandakan::application::ports::McpError;

    let parent = tempfile::tempdir().unwrap();
    std::fs::write(parent.path().join("secret.txt"), b"top secret").unwrap();
    let child = tempfile::Builder::new()
        .prefix("child")
        .tempdir_in(parent.path())
        .unwrap();

    let (_, read_tool, _, _) = build_fs_tools(child.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = read_tool.execute(&json!({ "path": "../secret.txt" })).await;

    assert!(matches!(result, Err(McpError::ExecutionFailed(_))));
    if let Err(McpError::ExecutionFailed(msg)) = result {
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

    let (list_tool, _, _, _) = build_fs_tools(child.path().to_str().unwrap(), 32_768, 200).unwrap();

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
fn given_read_file_schema_when_inspected_then_has_optional_line_range_parameters() {
    let schema = ReadFileTool::tool_schema();
    assert_eq!(schema.name, "read_file");
    assert!(schema.parameters["properties"]["path"].is_object());
    assert!(schema.parameters["properties"]["start_line"].is_object());
    assert!(schema.parameters["properties"]["end_line"].is_object());
    let required = schema.parameters["required"]
        .as_array()
        .expect("required must be an array");
    assert!(required.iter().any(|v| v.as_str() == Some("path")));
    assert!(!required.iter().any(|v| v.as_str() == Some("start_line")));
}

#[test]
fn given_search_files_schema_when_inspected_then_has_context_lines_parameter() {
    let schema = SearchFilesTool::tool_schema();
    assert_eq!(schema.name, "search_files");
    assert!(schema.parameters["properties"]["context_lines"].is_object());
}

#[tokio::test]
async fn given_list_directory_tool_when_querying_tool_name_then_returns_list_directory() {
    let dir = make_temp_tree();
    let (list_tool, _, _, _) = make_tools(&dir);
    assert_eq!(list_tool.tool_name(), "list_directory");
}

#[tokio::test]
async fn given_read_file_tool_when_querying_tool_name_then_returns_read_file() {
    let dir = make_temp_tree();
    let (_, read_tool, _, _) = make_tools(&dir);
    assert_eq!(read_tool.tool_name(), "read_file");
}

// ─── Construction error test ──────────────────────────────────────────────────

#[test]
fn given_nonexistent_root_path_when_building_fs_tools_then_returns_mcp_error() {
    use sandakan::application::ports::McpError;

    let result = build_fs_tools("/nonexistent/path/that/does/not/exist", 32_768, 200);
    assert!(matches!(result, Err(McpError::ExecutionFailed(_))));
}

// ─── list_directory depth tests ──────────────────────────────────────────────

#[tokio::test]
async fn given_depth_two_when_listing_directory_then_returns_nested_tree_entries() {
    let dir = make_temp_tree(); // has subdir/nested.txt
    let (list_tool, _, _, _) = make_tools(&dir);

    let result = list_tool
        .execute(&json!({ "path": ".", "depth": 2 }))
        .await
        .unwrap();

    assert!(result.contains("nested.txt"), "expected nested.txt in tree output: {result}");
    assert!(result.contains("subdir"), "expected subdir in tree output: {result}");
}

#[tokio::test]
async fn given_depth_exceeds_max_when_listing_then_depth_is_clamped_to_max_tree_depth() {
    let dir = make_temp_tree();
    let (list_tool, _, _, _) = make_tools(&dir);

    // depth=999 should not panic or hang; it gets clamped to max_tree_depth=5
    let result = list_tool
        .execute(&json!({ "path": ".", "depth": 999 }))
        .await;

    assert!(result.is_ok(), "expected Ok even with huge depth: {result:?}");
}

#[tokio::test]
async fn given_default_depth_when_listing_directory_then_returns_flat_list_only() {
    let dir = make_temp_tree(); // subdir/nested.txt exists
    let (list_tool, _, _, _) = make_tools(&dir);

    let result = list_tool
        .execute(&json!({ "path": "." }))
        .await
        .unwrap();

    // Default depth=1: subdir appears but nested.txt should NOT be in output
    assert!(result.contains("subdir"), "subdir should appear");
    assert!(!result.contains("nested.txt"), "nested.txt should not appear at depth=1");
}

// ─── search_files gitignore and size cap tests ────────────────────────────────

#[tokio::test]
async fn given_gitignore_excludes_file_when_searching_then_excluded_file_not_in_results() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("included.txt"), b"find_me here").unwrap();
    std::fs::write(dir.path().join("excluded.txt"), b"find_me here").unwrap();
    std::fs::write(dir.path().join(".gitignore"), b"excluded.txt\n").unwrap();
    let (_, _, search_tool, _) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = search_tool
        .execute(&json!({ "pattern": "find_me" }))
        .await
        .unwrap();

    assert!(result.contains("included.txt"), "included.txt should be found");
    assert!(!result.contains("excluded.txt"), "excluded.txt should be ignored by .gitignore");
}

#[tokio::test]
async fn given_file_exceeds_max_read_bytes_when_searching_then_file_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    // Write a file larger than max_read_bytes (10 bytes)
    let large_content = "find_me\n".repeat(10); // 80 bytes
    std::fs::write(dir.path().join("large.txt"), large_content.as_bytes()).unwrap();
    let (_, _, search_tool, _) = build_fs_tools(dir.path().to_str().unwrap(), 10, 200).unwrap();

    let result = search_tool
        .execute(&json!({ "pattern": "find_me" }))
        .await
        .unwrap();

    assert!(result.contains("No matches"), "large file should be skipped: {result}");
}

// ─── get_function_signatures broadened Rust patterns ─────────────────────────

#[tokio::test]
async fn given_rust_file_with_structs_and_enums_when_getting_signatures_then_returns_type_definitions() {
    let dir = tempfile::tempdir().unwrap();
    let src = b"pub struct Foo {\n    x: i32,\n}\n\npub enum Bar {\n    A,\n    B,\n}\n\ntype Alias = i32;\n";
    std::fs::write(dir.path().join("types.rs"), src).unwrap();
    let (_, _, _, sig_tool) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = sig_tool
        .execute(&json!({ "path": "types.rs" }))
        .await
        .unwrap();

    assert!(result.contains("struct Foo"), "expected struct Foo: {result}");
    assert!(result.contains("enum Bar"), "expected enum Bar: {result}");
    assert!(result.contains("type Alias"), "expected type Alias: {result}");
}

#[tokio::test]
async fn given_rust_file_with_impl_blocks_when_getting_signatures_then_returns_impl_declarations() {
    let dir = tempfile::tempdir().unwrap();
    let src = b"pub trait MyTrait {\n    fn method(&self);\n}\n\nimpl MyTrait for Foo {\n    fn method(&self) {}\n}\n\nimpl Foo {\n    pub fn new() -> Self { Foo { x: 0 } }\n}\n";
    std::fs::write(dir.path().join("impls.rs"), src).unwrap();
    let (_, _, _, sig_tool) = build_fs_tools(dir.path().to_str().unwrap(), 32_768, 200).unwrap();

    let result = sig_tool
        .execute(&json!({ "path": "impls.rs" }))
        .await
        .unwrap();

    assert!(result.contains("trait MyTrait"), "expected trait MyTrait: {result}");
    assert!(result.contains("impl MyTrait"), "expected impl MyTrait: {result}");
    assert!(result.contains("impl Foo"), "expected impl Foo: {result}");
}
