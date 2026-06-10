use encoding_rs::{GBK, WINDOWS_1252};

use super::model::{ComposeError, ComposeResult, FailureCase};

pub fn decode_text(raw: &[u8], label: &str) -> ComposeResult<String> {
    if raw.contains(&0) {
        return Err(ComposeError::new(
            "binary_refused",
            Some(FailureCase::Binary),
            format!("Binary input refused: {label}"),
        ));
    }

    if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(raw[3..].to_vec()).map_err(|_| {
            ComposeError::new(
                "invalid_encoding",
                Some(FailureCase::Encoding),
                format!("Invalid UTF-8 input: {label}"),
            )
        });
    }

    if let Ok(text) = String::from_utf8(raw.to_vec()) {
        return Ok(text);
    }

    let (decoded, _, had_errors) = GBK.decode(raw);
    if !had_errors {
        return Ok(decoded.into_owned());
    }

    let (decoded, _, had_errors) = WINDOWS_1252.decode(raw);
    if !had_errors {
        return Ok(decoded.into_owned());
    }

    Ok(String::from_utf8_lossy(raw).into_owned())
}

pub fn utf8_bytes(text: &str) -> Vec<u8> {
    text.as_bytes().to_vec()
}

// Keep trailing newlines attached to each segment so line-based transforms avoid
// silently normalizing LF/CRLF-shaped source text more than necessary.
pub fn split_line_segments(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\n' {
            segments.push(&text[start..=index]);
            start = index + 1;
        }
        index += 1;
    }
    if start < text.len() {
        segments.push(&text[start..]);
    }
    segments
}

pub fn select_lines(text: &str, start: usize, end: usize) -> ComposeResult<String> {
    let lines = split_line_segments(text);
    if lines.is_empty() {
        if start == 1 && end == 1 {
            return Ok(String::new());
        }
        return Err(range_error("line range out of bounds"));
    }
    if start == 0 || end == 0 || start > end || end > lines.len() {
        return Err(range_error("line range out of bounds"));
    }
    Ok(lines[(start - 1)..end].concat())
}

pub fn head_lines(text: &str, count: usize) -> String {
    split_line_segments(text)
        .into_iter()
        .take(count)
        .collect::<Vec<_>>()
        .concat()
}

pub fn tail_lines(text: &str, count: usize) -> String {
    let lines = split_line_segments(text);
    let start = lines.len().saturating_sub(count);
    lines[start..].concat()
}

pub fn select_chars(text: &str, start: usize, end: usize) -> ComposeResult<String> {
    let chars = text.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        if start == 1 && end == 1 {
            return Ok(String::new());
        }
        return Err(range_error("character range out of bounds"));
    }
    if start == 0 || end == 0 || start > end || end > chars.len() {
        return Err(range_error("character range out of bounds"));
    }
    Ok(chars[(start - 1)..end].iter().collect())
}

pub fn head_chars(text: &str, count: usize) -> String {
    text.chars().take(count).collect()
}

pub fn tail_chars(text: &str, count: usize) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let start = chars.len().saturating_sub(count);
    chars[start..].iter().collect()
}

pub fn indent(text: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    split_line_segments(text)
        .into_iter()
        .map(|line| format!("{prefix}{line}"))
        .collect::<String>()
}

pub fn oneline(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn limit_lines(text: &str, max: usize, fail: bool) -> ComposeResult<(String, bool)> {
    let lines = split_line_segments(text);
    if lines.len() <= max {
        return Ok((text.to_string(), false));
    }
    if fail {
        return Err(ComposeError::new(
            "limit_exceeded",
            Some(FailureCase::Limit),
            format!("Line limit exceeded: {max}"),
        ));
    }
    let mut out = lines.into_iter().take(max).collect::<Vec<_>>().concat();
    out.push_str(&format!("[asq compose: truncated after {max} lines]\n"));
    Ok((out, true))
}

// Byte limits operate on UTF-8 output bytes; walk back to a char boundary so the
// truncation marker never follows invalid UTF-8.
pub fn limit_bytes(text: &str, max: usize, fail: bool) -> ComposeResult<(String, bool)> {
    if text.len() <= max {
        return Ok((text.to_string(), false));
    }
    if fail {
        return Err(ComposeError::new(
            "limit_exceeded",
            Some(FailureCase::Limit),
            format!("Byte limit exceeded: {max}"),
        ));
    }

    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = text[..end].to_string();
    out.push_str(&format!("[asq compose: truncated after {max} bytes]\n"));
    Ok((out, true))
}

fn range_error(message: &str) -> ComposeError {
    ComposeError::new("invalid_range", Some(FailureCase::Range), message)
}
