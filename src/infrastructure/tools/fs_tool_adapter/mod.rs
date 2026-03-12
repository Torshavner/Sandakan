// @AI-BYPASS-LENGTH
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::application::ports::{McpError, ToolSchema};
use crate::infrastructure::mcp::ToolHandler;

mod list;
mod read;
mod search;
mod signatures;

pub struct FsToolInner {
    pub(super) root: PathBuf,
    pub(super) max_read_bytes: usize,
    pub(super) max_dir_entries: usize,
    pub(super) max_tree_depth: u32,
}

impl FsToolInner {
    pub fn new(
        root_path: &str,
        max_read_bytes: usize,
        max_dir_entries: usize,
    ) -> Result<Self, McpError> {
        let raw = PathBuf::from(root_path);
        let root = raw
            .canonicalize()
            .map_err(|e| McpError::ExecutionFailed(format!("fs_tools.root_path invalid: {e}")))?;
        Ok(Self {
            root,
            max_read_bytes,
            max_dir_entries,
            max_tree_depth: 5,
        })
    }

    pub(super) fn resolve_safe(&self, raw: &str) -> Result<PathBuf, McpError> {
        let joined = self.root.join(raw);
        let canonical = joined
            .canonicalize()
            .map_err(|_| McpError::ExecutionFailed(format!("path not found: {raw}")))?;
        if !canonical.starts_with(&self.root) {
            return Err(McpError::ExecutionFailed(
                "path escapes root boundary".to_string(),
            ));
        }
        Ok(canonical)
    }
}

// ─── Three thin newtype wrappers sharing one FsToolInner ─────────────────────

pub struct ListDirectoryTool(Arc<FsToolInner>);
pub struct ReadFileTool(Arc<FsToolInner>);
pub struct SearchFilesTool(Arc<FsToolInner>);
pub struct GetFunctionSignaturesTool(Arc<FsToolInner>);

impl ListDirectoryTool {
    pub fn tool_schema() -> ToolSchema {
        ToolSchema {
            name: "list_directory".to_string(),
            description: "List the contents of a directory within the configured root path. \
                Use path \".\" to list the root. Optionally pass depth > 1 for a recursive tree view."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path from the root to list. Use \".\" for root."
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Tree depth to recurse into subdirectories (default 1 = flat list, max 5)."
                    }
                },
                "required": ["path"]
            }),
        }
    }
}

impl ReadFileTool {
    pub fn tool_schema() -> ToolSchema {
        ToolSchema {
            name: "read_file".to_string(),
            description: "Read the contents of a file within the configured root path. \
                Supports optional start_line and end_line (1-based, inclusive) to read a specific \
                chunk instead of the whole file. Large files are truncated automatically."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path from the root to the file."
                    },
                    "start_line": {
                        "type": "integer",
                        "description": "First line to return (1-based, inclusive). Omit to start from the beginning."
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "Last line to return (1-based, inclusive). Omit to read to the end of the file."
                    }
                },
                "required": ["path"]
            }),
        }
    }
}

impl SearchFilesTool {
    pub fn tool_schema() -> ToolSchema {
        ToolSchema {
            name: "search_files".to_string(),
            description: "Search for a regex pattern across all files under the configured root. \
                By default returns matching lines with file path and line number (max 10). \
                Set files_only=true to return only the file paths that contain a match — \
                much cheaper for locating relevant files before reading them with read_file. \
                Respects .gitignore and skips hidden files."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern to search for."
                    },
                    "path": {
                        "type": "string",
                        "description": "Optional subdirectory to limit the search to (relative to root)."
                    },
                    "max_matches": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default 10)."
                    },
                    "context_lines": {
                        "type": "integer",
                        "description": "Number of lines to show before and after each match (default 0). Ignored when files_only=true."
                    },
                    "files_only": {
                        "type": "boolean",
                        "description": "When true, return only file paths that contain a match — no line numbers or content. Use this first to locate files cheaply, then read_file to inspect them."
                    }
                },
                "required": ["pattern"]
            }),
        }
    }
}

impl GetFunctionSignaturesTool {
    pub fn tool_schema() -> ToolSchema {
        ToolSchema {
            name: "get_function_signatures".to_string(),
            description:
                "Return code definitions (functions, structs, enums, traits, impl blocks) \
                from a source file, without reading the full file body. \
                Supports Rust, Python, JavaScript, TypeScript, Go, Java, Kotlin, C, and C++. \
                Use this instead of read_file when you only need to know what definitions exist."
                    .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path from the root to the source file."
                    }
                },
                "required": ["path"]
            }),
        }
    }
}

/// Constructs all four fs tools sharing one `FsToolInner`.
pub fn build_fs_tools(
    root_path: &str,
    max_read_bytes: usize,
    max_dir_entries: usize,
) -> Result<
    (
        ListDirectoryTool,
        ReadFileTool,
        SearchFilesTool,
        GetFunctionSignaturesTool,
    ),
    McpError,
> {
    let inner = Arc::new(FsToolInner::new(
        root_path,
        max_read_bytes,
        max_dir_entries,
    )?);
    Ok((
        ListDirectoryTool(Arc::clone(&inner)),
        ReadFileTool(Arc::clone(&inner)),
        SearchFilesTool(Arc::clone(&inner)),
        GetFunctionSignaturesTool(inner),
    ))
}

#[async_trait]
impl ToolHandler for ListDirectoryTool {
    fn tool_name(&self) -> &str {
        "list_directory"
    }

    async fn execute(&self, arguments: &serde_json::Value) -> Result<String, McpError> {
        self.0.list(arguments).await
    }
}

#[async_trait]
impl ToolHandler for ReadFileTool {
    fn tool_name(&self) -> &str {
        "read_file"
    }

    async fn execute(&self, arguments: &serde_json::Value) -> Result<String, McpError> {
        self.0.read(arguments).await
    }
}

#[async_trait]
impl ToolHandler for SearchFilesTool {
    fn tool_name(&self) -> &str {
        "search_files"
    }

    async fn execute(&self, arguments: &serde_json::Value) -> Result<String, McpError> {
        self.0.search(arguments).await
    }
}

#[async_trait]
impl ToolHandler for GetFunctionSignaturesTool {
    fn tool_name(&self) -> &str {
        "get_function_signatures"
    }

    async fn execute(&self, arguments: &serde_json::Value) -> Result<String, McpError> {
        self.0.get_function_signatures(arguments).await
    }
}
