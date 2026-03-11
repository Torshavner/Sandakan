use std::path::PathBuf;
use std::pin::Pin;

use super::FsToolInner;
use crate::application::ports::McpError;

impl FsToolInner {
    pub async fn list(&self, args: &serde_json::Value) -> Result<String, McpError> {
        let raw = args["path"]
            .as_str()
            .ok_or_else(|| McpError::Serialization("missing 'path' argument".to_string()))?;

        let dir = self.resolve_safe(raw)?;

        let depth_arg = args["depth"].as_u64().unwrap_or(1) as u32;
        let depth = depth_arg.min(self.max_tree_depth);

        if depth <= 1 {
            return self.list_flat(&dir).await;
        }

        let relative = dir.strip_prefix(&self.root).unwrap_or(&dir);
        let display = relative.display();
        let mut lines: Vec<String> = Vec::new();
        let mut entry_count = 0usize;
        self.tree_recursive(&dir, "", depth - 1, &mut lines, &mut entry_count)
            .await?;

        Ok(format!(
            "Directory: {}/{} (tree depth {})\n{}",
            self.root.display(),
            display,
            depth,
            lines.join("\n")
        ))
    }

    async fn list_flat(&self, dir: &PathBuf) -> Result<String, McpError> {
        let mut read_dir = tokio::fs::read_dir(dir)
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

        let relative = dir.strip_prefix(&self.root).unwrap_or(dir);
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

    pub(super) fn tree_recursive<'a>(
        &'a self,
        dir: &'a PathBuf,
        prefix: &'a str,
        depth: u32,
        lines: &'a mut Vec<String>,
        entry_count: &'a mut usize,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), McpError>> + Send + 'a>> {
        Box::pin(self.tree_recursive_inner(dir, prefix, depth, lines, entry_count))
    }

    async fn tree_recursive_inner(
        &self,
        dir: &PathBuf,
        prefix: &str,
        depth: u32,
        lines: &mut Vec<String>,
        entry_count: &mut usize,
    ) -> Result<(), McpError> {
        let mut read_dir = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("cannot read directory: {e}")))?;

        // Collect all entries first so we can detect the last one.
        let mut entries_raw: Vec<(String, PathBuf, bool)> = Vec::new();
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("directory read error: {e}")))?
        {
            if *entry_count >= self.max_dir_entries {
                break;
            }
            let ft = entry
                .file_type()
                .await
                .map_err(|e| McpError::ExecutionFailed(format!("file type error: {e}")))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            entries_raw.push((name, entry.path(), ft.is_dir()));
            *entry_count += 1;
        }

        entries_raw.sort_by(|a, b| a.0.cmp(&b.0));

        let total = entries_raw.len();
        for (idx, (name, path, is_dir)) in entries_raw.into_iter().enumerate() {
            let is_last = idx + 1 == total;
            let connector = if is_last { "└── " } else { "├── " };
            lines.push(format!("{prefix}{connector}{name}"));

            if is_dir && depth > 0 {
                let child_prefix = if is_last {
                    format!("{prefix}    ")
                } else {
                    format!("{prefix}│   ")
                };
                self.tree_recursive(&path, &child_prefix, depth - 1, lines, entry_count)
                    .await?;
            }
        }

        Ok(())
    }
}
