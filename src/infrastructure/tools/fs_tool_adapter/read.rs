use super::FsToolInner;
use crate::application::ports::McpError;

impl FsToolInner {
    /// Reads a file, optionally restricted to a line range (1-based, inclusive).
    pub async fn read(&self, args: &serde_json::Value) -> Result<String, McpError> {
        let raw = args["path"]
            .as_str()
            .ok_or_else(|| McpError::Serialization("missing 'path' argument".to_string()))?;

        let path = self.resolve_safe(raw)?;

        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("cannot read file: {e}")))?;

        let total_bytes = bytes.len();
        let slice = if total_bytes > self.max_read_bytes {
            &bytes[..self.max_read_bytes]
        } else {
            &bytes
        };

        let text = std::str::from_utf8(slice)
            .map_err(|_| McpError::ExecutionFailed("binary file".to_string()))?;

        let start_line = args["start_line"].as_u64().map(|v| v as usize);
        let end_line = args["end_line"].as_u64().map(|v| v as usize);

        if start_line.is_none() && end_line.is_none() {
            // No line range requested — return as before.
            if total_bytes > self.max_read_bytes {
                return Ok(format!(
                    "{text}\n[truncated — {total_bytes} bytes total, showing first {}]",
                    self.max_read_bytes
                ));
            }
            return Ok(text.to_string());
        }

        // Apply line-range slice (1-based, inclusive).
        let lines: Vec<&str> = text.lines().collect();
        let total_lines = lines.len();

        let start = start_line.map(|n| n.saturating_sub(1)).unwrap_or(0);
        let end = end_line.map(|n| n.min(total_lines)).unwrap_or(total_lines);

        if start >= total_lines {
            return Err(McpError::ExecutionFailed(format!(
                "start_line {start_line:?} exceeds file length ({total_lines} lines)"
            )));
        }

        if start > end {
            return Err(McpError::ExecutionFailed(format!(
                "start_line ({}) must not be greater than end_line ({})",
                start + 1,
                end
            )));
        }

        let selected: Vec<String> = lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{}: {}", start + i + 1, line))
            .collect();

        let byte_truncation = if total_bytes > self.max_read_bytes {
            format!(
                "\n[note: file was byte-truncated at {}; line range may be incomplete]",
                self.max_read_bytes
            )
        } else {
            String::new()
        };

        Ok(format!(
            "{}:{}-{} ({} lines){}\n{}",
            raw,
            start + 1,
            end,
            selected.len(),
            byte_truncation,
            selected.join("\n")
        ))
    }
}
