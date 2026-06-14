pub fn strip_line_ending(line: &str) -> &str {
    if let Some(s) = line.strip_suffix("\r\n") {
        s
    } else if let Some(s) = line.strip_suffix('\n') {
        s
    } else if let Some(s) = line.strip_suffix('\r') {
        s
    } else {
        line
    }
}

pub fn detect_newline_style(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else if text.contains('\r') {
        "\r"
    } else {
        "\n"
    }
}

pub fn convert_newlines(text: &str, newline: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', newline)
}

pub fn split_lines_keepends(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'\r' {
            let end = if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                i + 2
            } else {
                i + 1
            };
            lines.push(text[start..end].to_string());
            i = end;
            start = end;
        } else if bytes[i] == b'\n' {
            let end = i + 1;
            lines.push(text[start..end].to_string());
            i = end;
            start = end;
        } else {
            i += 1;
        }
    }

    if start < text.len() {
        lines.push(text[start..].to_string());
    }

    lines
}

pub fn norm_line_exact(line: &str) -> String {
    strip_line_ending(line).to_string()
}

pub fn norm_line_loose(line: &str) -> String {
    let s = strip_line_ending(line).trim_end_matches([' ', '\t']);
    if s.trim().is_empty() {
        String::new()
    } else {
        s.to_string()
    }
}

/// Prepend `delta` to each line that has non-empty content
/// (after stripping line endings). Empty lines are left unchanged.
pub fn adjust_line_indent(lines: &[String], delta: &str) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            let stripped = strip_line_ending(line);
            if stripped.is_empty() {
                line.clone()
            } else {
                let ending = &line[stripped.len()..];
                format!("{delta}{stripped}{ending}")
            }
        })
        .collect()
}
