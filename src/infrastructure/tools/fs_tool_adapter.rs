use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::application::ports::{McpError, ToolSchema};
use crate::infrastructure::mcp::ToolHandler;

pub struct FsToolInner {
    root: PathBuf,
    max_read_bytes: usize,
    max_dir_entries: usize,
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
        })
    }

    fn resolve_safe(&self, raw: &str) -> Result<PathBuf, McpError> {
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

    pub async fn list(&self, args: &serde_json::Value) -> Result<String, McpError> {
        let raw = args["path"]
            .as_str()
            .ok_or_else(|| McpError::Serialization("missing 'path' argument".to_string()))?;

        let dir = self.resolve_safe(raw)?;

        let mut read_dir = tokio::fs::read_dir(&dir)
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("cannot read directory: {e}")))?;

        let mut entries: Vec<String> = Vec::new();
        let mut total = 0usize;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("directory read error: {e}")))?
        {
            if total >= self.max_dir_entries {
                break;
            }
            let ft = entry
                .file_type()
                .await
                .map_err(|e| McpError::ExecutionFailed(format!("file type error: {e}")))?;
            let tag = if ft.is_dir() { "[DIR] " } else { "[FILE]" };
            let name = entry.file_name().to_string_lossy().into_owned();
            entries.push(format!("{tag} {name}"));
            total += 1;
        }

        entries.sort();

        let relative = dir.strip_prefix(&self.root).unwrap_or(&dir);
        let display = relative.display();
        let truncation_note = if total >= self.max_dir_entries {
            format!(" (truncated at {} entries)", self.max_dir_entries)
        } else {
            String::new()
        };

        Ok(format!(
            "Directory: {}/{} ({} entries{})\n{}",
            self.root.display(),
            display,
            total,
            truncation_note,
            entries.join("\n")
        ))
    }

    pub async fn read(&self, args: &serde_json::Value) -> Result<String, McpError> {
        let raw = args["path"]
            .as_str()
            .ok_or_else(|| McpError::Serialization("missing 'path' argument".to_string()))?;

        let path = self.resolve_safe(raw)?;

        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("cannot read file: {e}")))?;

        let total = bytes.len();
        let slice = if total > self.max_read_bytes {
            &bytes[..self.max_read_bytes]
        } else {
            &bytes
        };

        let text = std::str::from_utf8(slice)
            .map_err(|_| McpError::ExecutionFailed("binary file".to_string()))?;

        if total > self.max_read_bytes {
            Ok(format!(
                "{text}\n[truncated — {total} bytes total, showing first {}]",
                self.max_read_bytes
            ))
        } else {
            Ok(text.to_string())
        }
    }
}

// ─── Two thin newtype wrappers sharing one FsToolInner ────────────────────────

pub struct ListDirectoryTool(Arc<FsToolInner>);
pub struct ReadFileTool(Arc<FsToolInner>);

impl ListDirectoryTool {
    pub fn tool_schema() -> ToolSchema {
        ToolSchema {
            name: "list_directory".to_string(),
            description: "List the contents of a directory within the configured root path. \
                Use path \".\" to list the root."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path from the root to list. Use \".\" for root."
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
                Large files are truncated automatically."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path from the root to the file."
                    }
                },
                "required": ["path"]
            }),
        }
    }
}

/// Constructs a matched pair of `(ListDirectoryTool, ReadFileTool)` sharing one inner.
pub fn build_fs_tools(
    root_path: &str,
    max_read_bytes: usize,
    max_dir_entries: usize,
) -> Result<(ListDirectoryTool, ReadFileTool), McpError> {
    let inner = Arc::new(FsToolInner::new(
        root_path,
        max_read_bytes,
        max_dir_entries,
    )?);
    Ok((ListDirectoryTool(Arc::clone(&inner)), ReadFileTool(inner)))
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
