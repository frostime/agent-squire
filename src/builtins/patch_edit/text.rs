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

pub fn is_blank_line(line: &str) -> bool {
    strip_line_ending(line)
        .chars()
        .all(|c| c == ' ' || c == '\t')
}

pub fn common_base_indent(lines: &[String]) -> String {
    let mut common: Option<String> = None;

    for line in lines.iter().filter(|line| !is_blank_line(line)) {
        let indent = leading_whitespace(strip_line_ending(line));
        common = Some(match common {
            None => indent.to_string(),
            Some(prev) => common_prefix(&prev, indent).to_string(),
        });
    }

    common.unwrap_or_default()
}

pub fn strip_base_indent(lines: &[String], base: &str) -> Option<Vec<String>> {
    lines
        .iter()
        .map(|line| strip_base_indent_line(line, base))
        .collect()
}

pub fn migrate_base_indent(lines: &[String], from: &str, to: &str) -> Option<Vec<String>> {
    lines
        .iter()
        .map(|line| {
            if is_blank_line(line) {
                Some(line.clone())
            } else {
                strip_base_indent_line(line, from).map(|stripped| format!("{to}{stripped}"))
            }
        })
        .collect()
}

fn strip_base_indent_line(line: &str, base: &str) -> Option<String> {
    if is_blank_line(line) {
        Some(line.to_string())
    } else {
        line.strip_prefix(base).map(ToString::to_string)
    }
}

fn leading_whitespace(line: &str) -> &str {
    let end = line
        .char_indices()
        .find_map(|(idx, ch)| (ch != ' ' && ch != '\t').then_some(idx))
        .unwrap_or(line.len());
    &line[..end]
}

fn common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let mut end = 0usize;
    for ((a_idx, a_ch), (_, b_ch)) in a.char_indices().zip(b.char_indices()) {
        if a_ch != b_ch {
            break;
        }
        end = a_idx + a_ch.len_utf8();
    }
    &a[..end]
}
