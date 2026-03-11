use regex::Regex;

use super::FsToolInner;
use crate::application::ports::McpError;

impl FsToolInner {
    /// Returns code definitions found in a file using language-specific regex patterns.
    pub async fn get_function_signatures(
        &self,
        args: &serde_json::Value,
    ) -> Result<String, McpError> {
        let raw = args["path"]
            .as_str()
            .ok_or_else(|| McpError::Serialization("missing 'path' argument".to_string()))?;

        let path = self.resolve_safe(raw)?;

        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| McpError::ExecutionFailed(format!("cannot read file: {e}")))?;

        let text = std::str::from_utf8(&bytes)
            .map_err(|_| McpError::ExecutionFailed("binary file".to_string()))?;

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let signatures = extract_signatures(text, &ext);

        if signatures.is_empty() {
            Ok(format!(
                "No code definitions found in {raw} (detected language: {ext})"
            ))
        } else {
            Ok(format!(
                "Code definitions in {} ({} found):\n{}",
                raw,
                signatures.len(),
                signatures.join("\n")
            ))
        }
    }
}

/// Extracts code definitions from source text using language-specific patterns.
/// Returns lines formatted as `line_number: definition`.
pub(super) fn extract_signatures(text: &str, ext: &str) -> Vec<String> {
    let patterns: &[&str] = match ext {
        "rs" => &[
            r"^\s*(pub(?:\([^)]*\))?\s+)?(async\s+)?fn\s+\w+",
            r"^\s*(pub(?:\([^)]*\))?\s+)?struct\s+\w+",
            r"^\s*(pub(?:\([^)]*\))?\s+)?enum\s+\w+",
            r"^\s*(pub(?:\([^)]*\))?\s+)?trait\s+\w+",
            r"^\s*(pub(?:\([^)]*\))?\s+)?type\s+\w+",
            r"^\s*impl(\s*<[^>]*>)?\s+\w+",
        ],
        "py" => &[r"^\s*(async\s+)?def\s+\w+"],
        "js" | "mjs" | "cjs" => &[
            r"^\s*(async\s+)?function\s+\w+",
            r"^\s*(export\s+)?(const|let|var)\s+\w+\s*=\s*(async\s*)?\(",
            r"^\s*(async\s+)?\w+\s*\([^)]*\)\s*\{",
        ],
        "ts" | "tsx" => &[
            r"^\s*(export\s+)?(async\s+)?function\s+\w+",
            r"^\s*(public|private|protected|static|async|\s)+\w+\s*\(",
            r"^\s*(export\s+)?(const|let)\s+\w+\s*=\s*(async\s*)?\(",
        ],
        "go" => &[r"^\s*func\s+(\(\s*\w+\s+\*?\w+\s*\)\s*)?\w+\s*\("],
        "java" | "kt" => &[
            r"^\s*(public|private|protected|static|final|abstract|synchronized|\s)*(void|\w+)\s+\w+\s*\(",
        ],
        "c" | "cpp" | "cc" | "h" | "hpp" => &[r"^\s*(\w[\w\s\*&:<>]*)\s+\w+\s*\([^;]*$"],
        _ => &[
            r"^\s*(?!(?:if|for|while|switch|catch)\s*\()[a-zA-Z_]\w*\s*\(",
        ],
    };

    let compiled: Vec<Regex> = patterns.iter().filter_map(|p| Regex::new(p).ok()).collect();

    text.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with('*')
                || trimmed.starts_with("/*")
            {
                return None;
            }
            if compiled.iter().any(|re| re.is_match(line)) {
                Some(format!("{}: {}", i + 1, line.trim_end()))
            } else {
                None
            }
        })
        .collect()
}
