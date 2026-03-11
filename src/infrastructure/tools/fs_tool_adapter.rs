use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use regex::Regex;

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

    pub async fn search(&self, args: &serde_json::Value) -> Result<String, McpError> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| McpError::Serialization("missing 'pattern' argument".to_string()))?;

        let search_root = match args["path"].as_str() {
            Some(p) => self.resolve_safe(p)?,
            None => self.root.clone(),
        };

        let regex = Regex::new(pattern)
            .map_err(|e| McpError::ExecutionFailed(format!("invalid regex: {e}")))?;

        let max_matches = args["max_matches"].as_u64().unwrap_or(50) as usize;

        let mut matches: Vec<String> = Vec::new();
        self.grep_dir(&search_root, &regex, &mut matches, max_matches)
            .await?;

        if matches.is_empty() {
            Ok(format!("No matches found for pattern: {pattern}"))
        } else {
            Ok(format!(
                "{} match(es) for `{}`:\n{}",
                matches.len(),
                pattern,
                matches.join("\n")
            ))
        }
    }

    fn grep_dir<'a>(
        &'a self,
        dir: &'a PathBuf,
        regex: &'a Regex,
        matches: &'a mut Vec<String>,
        max_matches: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), McpError>> + Send + 'a>>
    {
        Box::pin(async move {
            let mut read_dir = tokio::fs::read_dir(dir)
                .await
                .map_err(|e| McpError::ExecutionFailed(format!("cannot read dir: {e}")))?;

            while let Some(entry) = read_dir
                .next_entry()
                .await
                .map_err(|e| McpError::ExecutionFailed(format!("dir read error: {e}")))?
            {
                if matches.len() >= max_matches {
                    break;
                }

                let path = entry.path();

                if !path.starts_with(&self.root) {
                    continue;
                }

                let ft = entry
                    .file_type()
                    .await
                    .map_err(|e| McpError::ExecutionFailed(format!("file type error: {e}")))?;

                if ft.is_dir() {
                    // Skip heavyweight/generated directories that are never useful to search.
                    let dir_name = entry.file_name();
                    let skip = matches!(
                        dir_name.to_str().unwrap_or(""),
                        "target" | ".git" | "node_modules" | ".next" | "dist" | "build"
                    );
                    if !skip {
                        self.grep_dir(&path, regex, matches, max_matches).await?;
                    }
                } else if ft.is_file() {
                    let bytes = match tokio::fs::read(&path).await {
                        Ok(b) => b,
                        Err(_) => continue,
                    };
                    let text = match std::str::from_utf8(&bytes) {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    let rel = path
                        .strip_prefix(&self.root)
                        .unwrap_or(&path)
                        .display()
                        .to_string();
                    for (line_no, line) in text.lines().enumerate() {
                        if matches.len() >= max_matches {
                            break;
                        }
                        if regex.is_match(line) {
                            matches.push(format!("{}:{}: {}", rel, line_no + 1, line));
                        }
                    }
                }
            }
            Ok(())
        })
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

// ─── Three thin newtype wrappers sharing one FsToolInner ─────────────────────

pub struct ListDirectoryTool(Arc<FsToolInner>);
pub struct ReadFileTool(Arc<FsToolInner>);
pub struct SearchFilesTool(Arc<FsToolInner>);

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

impl SearchFilesTool {
    pub fn tool_schema() -> ToolSchema {
        ToolSchema {
            name: "search_files".to_string(),
            description: "Search for a regex pattern across all files under the configured root. \
                Returns matching lines with file path and line number. \
                Use this to find function definitions, struct names, or any keyword."
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
                        "description": "Maximum number of matching lines to return (default 50)."
                    }
                },
                "required": ["pattern"]
            }),
        }
    }
}

/// Constructs a triple of `(ListDirectoryTool, ReadFileTool, SearchFilesTool)` sharing one inner.
pub fn build_fs_tools(
    root_path: &str,
    max_read_bytes: usize,
    max_dir_entries: usize,
) -> Result<(ListDirectoryTool, ReadFileTool, SearchFilesTool), McpError> {
    let inner = Arc::new(FsToolInner::new(
        root_path,
        max_read_bytes,
        max_dir_entries,
    )?);
    Ok((
        ListDirectoryTool(Arc::clone(&inner)),
        ReadFileTool(Arc::clone(&inner)),
        SearchFilesTool(inner),
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
