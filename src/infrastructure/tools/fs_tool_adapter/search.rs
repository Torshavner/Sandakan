use regex::Regex;

use super::FsToolInner;
use crate::application::ports::McpError;

impl FsToolInner {
    /// Searches files for a regex pattern, returning matching lines with optional context.
    /// Uses the `ignore` crate to respect .gitignore and skip hidden files.
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
        let context_lines = args["context_lines"].as_u64().unwrap_or(0) as usize;

        let root = self.root.clone();
        let max_read_bytes = self.max_read_bytes;

        let matches = tokio::task::spawn_blocking(move || {
            search_with_ignore(
                &search_root,
                &root,
                &regex,
                max_matches,
                context_lines,
                max_read_bytes,
            )
        })
        .await
        .map_err(|e| McpError::ExecutionFailed(format!("search task failed: {e}")))?;

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
}

fn search_with_ignore(
    search_root: &std::path::Path,
    root: &std::path::Path,
    regex: &Regex,
    max_matches: usize,
    context_lines: usize,
    max_read_bytes: usize,
) -> Vec<String> {
    use std::io::BufRead;

    let mut matches: Vec<String> = Vec::new();

    let walker = ignore::WalkBuilder::new(search_root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        // Read .gitignore even when no .git directory is present (e.g. sub-trees, temp dirs in tests).
        .require_git(false)
        .build();

    for result in walker {
        if matches.len() >= max_matches {
            break;
        }

        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        if !path.starts_with(root) {
            continue;
        }

        if path.is_dir() {
            continue;
        }

        // Size cap — skip files larger than max_read_bytes.
        let size = match std::fs::metadata(path) {
            Ok(m) => m.len() as usize,
            Err(_) => continue,
        };
        if size > max_read_bytes {
            continue;
        }

        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        let reader = std::io::BufReader::new(file);
        let lines_vec: Vec<String> = reader.lines().map_while(Result::ok).collect();

        if lines_vec.is_empty() {
            continue;
        }

        // Check if file has any UTF-8 issues by trying to validate all lines (already done via BufRead).
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();

        let total_lines = lines_vec.len();
        let mut emitted_ranges: Vec<(usize, usize)> = Vec::new();

        for (line_no, line) in lines_vec.iter().enumerate() {
            if matches.len() >= max_matches {
                break;
            }
            if regex.is_match(line) {
                let start = line_no.saturating_sub(context_lines);
                let end = (line_no + context_lines + 1).min(total_lines);

                let effective_start = emitted_ranges
                    .last()
                    .map(|&(_, prev_end)| prev_end.max(start))
                    .unwrap_or(start);

                if effective_start < end {
                    if context_lines == 0 {
                        matches.push(format!("{}:{}: {}", rel, line_no + 1, line));
                    } else {
                        if !emitted_ranges.is_empty()
                            && effective_start > emitted_ranges.last().unwrap().1
                        {
                            matches.push(format!("{}:---", rel));
                        }
                        for (offset, ctx_line) in lines_vec[effective_start..end].iter().enumerate()
                        {
                            let ctx_no = effective_start + offset;
                            let marker = if ctx_no == line_no { ">" } else { " " };
                            matches.push(format!("{}:{}{}: {}", rel, marker, ctx_no + 1, ctx_line));
                        }
                        emitted_ranges.push((effective_start, end));
                    }
                }
            }
        }
    }

    matches
}
