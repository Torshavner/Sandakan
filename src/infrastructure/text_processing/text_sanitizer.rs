use regex::Regex;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

static HYPHEN_NEWLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\w)-\n(\w)").unwrap());

pub fn sanitize_extracted_text(raw: &str) -> String {
    let normalized: String = raw.nfkc().collect();
    let dehyphenated = HYPHEN_NEWLINE.replace_all(&normalized, "$1$2");

    let mut result = String::with_capacity(dehyphenated.len());
    let mut prev_was_blank = false;
    let mut first_content = true;

    for line in dehyphenated.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            prev_was_blank = true;
        } else {
            if !first_content && prev_was_blank {
                result.push_str("\n\n");
            } else if !first_content {
                result.push('\n');
            }
            collapse_internal_whitespace(trimmed, &mut result);
            prev_was_blank = false;
            first_content = false;
        }
    }

    result.trim().to_string()
}

fn collapse_internal_whitespace(line: &str, out: &mut String) {
    let mut prev_was_space = false;

    for ch in line.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                out.push(' ');
                prev_was_space = true;
            }
        } else {
            out.push(ch);
            prev_was_space = false;
        }
    }
}
