use std::path::{Component, Path, PathBuf};

use regex::Regex;

use super::model::{PatchBlock, PatchOperation};
use super::text::{split_lines_keepends, strip_line_ending};

const SEARCH_MARK: &str = "<<<<<<< SEARCH";
const CREATE_MARK: &str = "<<<<<<< CREATE";
const OVERWRITE_MARK: &str = "<<<<<<< OVERWRITE";
const DELIM_MARK: &str = "=======";
const REPLACE_MARK: &str = ">>>>>>> REPLACE";

pub fn parse_patches(
    patch_text: &str,
    project_root: &Path,
) -> Result<Vec<PatchBlock>, Vec<String>> {
    let root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let lines = split_lines_keepends(patch_text);
    let roles = (0..lines.len())
        .map(|idx| classify_line(&lines, idx))
        .collect::<Vec<_>>();
    let schema = roles.iter().collect::<String>();
    let pattern = Regex::new(r"F(?P<gap>B*)S(?P<search>[BC]*?)D(?P<replace>[BC]*?)R").unwrap();

    let mut patches = Vec::new();
    let mut errors = Vec::new();

    for caps in pattern.captures_iter(&schema) {
        let full = caps.get(0).expect("full match");
        let file_line_idx = full.start();
        let file_line = strip_line_ending(&lines[file_line_idx]);
        let gap_len = caps.name("gap").map(|m| m.as_str().len()).unwrap_or(0);
        let opener_line_idx = file_line_idx + 1 + gap_len;
        let opener_line = strip_line_ending(&lines[opener_line_idx]);

        let (file_path, display_path, line_range) = match parse_patch_header(file_line, &root) {
            Ok(v) => v,
            Err(err) => {
                errors.push(format!(
                    "Line {}: Failed to parse patch header: {} ({err})",
                    file_line_idx + 1,
                    file_line
                ));
                continue;
            }
        };

        let operation = match parse_patch_operation(opener_line) {
            Some(op) => op,
            None => {
                errors.push(format!(
                    "Line {}: Invalid patch opener: {}",
                    opener_line_idx + 1,
                    opener_line
                ));
                continue;
            }
        };

        let search_range = caps.name("search").expect("search capture");
        let replace_range = caps.name("replace").expect("replace capture");
        let search_content = lines[search_range.start()..search_range.end()].concat();
        let replace_content = lines[replace_range.start()..replace_range.end()].concat();

        if let Some(err) = validate_patch_block(
            &operation,
            &display_path,
            &file_path,
            line_range,
            &search_content,
        ) {
            errors.push(format!("Line {}: {err}", file_line_idx + 1));
            continue;
        }

        patches.push(PatchBlock {
            file_path,
            display_path,
            operation,
            line_range,
            search_content,
            replace_content,
            source_line_start: file_line_idx + 1,
        });
    }

    if patch_text.trim().len() > 0 && patches.is_empty() && errors.is_empty() {
        errors.push("No valid patch blocks found. Ensure each block starts with '# <path>' followed by a SEARCH/REPLACE block.".into());
    }

    if errors.is_empty() {
        Ok(patches)
    } else {
        Err(errors)
    }
}

fn classify_line(lines: &[String], index: usize) -> char {
    let stripped = strip_line_ending(&lines[index]);

    if is_patch_header_line(lines, index) {
        return 'F';
    }

    match stripped {
        SEARCH_MARK | CREATE_MARK | OVERWRITE_MARK => 'S',
        DELIM_MARK => 'D',
        REPLACE_MARK => 'R',
        _ if stripped.trim().is_empty() => 'B',
        _ => 'C',
    }
}

fn is_patch_header_line(lines: &[String], index: usize) -> bool {
    let stripped = strip_line_ending(&lines[index]);
    if !stripped.starts_with("# ") {
        return false;
    }

    if parse_patch_header_text(stripped[2..].trim()).is_err() {
        return false;
    }

    let mut next = index + 1;
    while next < lines.len() && strip_line_ending(&lines[next]).trim().is_empty() {
        next += 1;
    }

    next < lines.len()
        && matches!(
            strip_line_ending(&lines[next]),
            SEARCH_MARK | CREATE_MARK | OVERWRITE_MARK
        )
}

fn parse_patch_operation(line: &str) -> Option<PatchOperation> {
    match line {
        SEARCH_MARK => Some(PatchOperation::Search),
        CREATE_MARK => Some(PatchOperation::Create),
        OVERWRITE_MARK => Some(PatchOperation::Overwrite),
        _ => None,
    }
}

fn validate_patch_block(
    operation: &PatchOperation,
    display_path: &str,
    file_path: &Path,
    line_range: Option<(Option<usize>, Option<usize>)>,
    search_content: &str,
) -> Option<String> {
    if file_path.exists() && !file_path.is_file() {
        return Some(format!("Not a file: {display_path}"));
    }

    if matches!(
        operation,
        PatchOperation::Create | PatchOperation::Overwrite
    ) {
        if line_range.is_some() {
            return Some(format!(
                "Line range is only supported for SEARCH patches, got {}: {display_path}",
                operation_name(operation)
            ));
        }

        if !search_content.trim().is_empty() {
            return Some(format!(
                "{} upper block must be whitespace-only: {display_path}",
                operation_name(operation)
            ));
        }
    }

    if matches!(operation, PatchOperation::Search) && !file_path.exists() {
        return Some(format!("File does not exist: {display_path}"));
    }

    None
}

fn operation_name(operation: &PatchOperation) -> &'static str {
    match operation {
        PatchOperation::Search => "SEARCH",
        PatchOperation::Create => "CREATE",
        PatchOperation::Overwrite => "OVERWRITE",
    }
}

fn parse_patch_header(
    header: &str,
    project_root: &Path,
) -> Result<(PathBuf, String, Option<(Option<usize>, Option<usize>)>), String> {
    let stripped = strip_line_ending(header);
    if !stripped.starts_with("# ") {
        return Err(format!("Invalid patch header: {header}"));
    }

    let (display_path, line_range) = parse_patch_header_text(stripped[2..].trim())?;
    let file_path = resolve_patch_path(project_root, &display_path)?;
    Ok((file_path, display_path, line_range))
}

fn parse_patch_header_text(
    text: &str,
) -> Result<(String, Option<(Option<usize>, Option<usize>)>), String> {
    if text.is_empty() {
        return Err(format!("Invalid patch header: {text}"));
    }

    if let Some(pos) = text.rfind(':') {
        let path_part = &text[..pos];
        let suffix = &text[pos + 1..];
        if path_part.is_empty() {
            return Err(format!("Invalid patch header: {text}"));
        }

        if let Ok(line_range) = parse_line_range(suffix) {
            return Ok((path_part.to_string(), Some(line_range)));
        }
    }

    Ok((text.to_string(), None))
}

pub fn parse_line_range(text: &str) -> Result<(Option<usize>, Option<usize>), String> {
    let parse_n = |s: &str| -> Result<usize, String> {
        let n = s
            .parse::<usize>()
            .map_err(|_| format!("Invalid line range: {text}"))?;
        if n == 0 {
            Err(format!("Invalid line range: {text}"))
        } else {
            Ok(n)
        }
    };

    let out = if let Some(rest) = text.strip_prefix('L') {
        if let Some((start, end)) = rest.split_once("-L") {
            (Some(parse_n(start)?), Some(parse_n(end)?))
        } else if let Some(start) = rest.strip_suffix('-') {
            (Some(parse_n(start)?), None)
        } else {
            return Err(format!("Invalid line range: {text}"));
        }
    } else if let Some(end) = text.strip_prefix("-L") {
        (None, Some(parse_n(end)?))
    } else if let Some((start, end)) = text.split_once('-') {
        (Some(parse_n(start)?), Some(parse_n(end)?))
    } else {
        return Err(format!("Invalid line range: {text}"));
    };

    if let (Some(start), Some(end)) = out {
        if end < start {
            return Err(format!("Invalid line range: {text}"));
        }
    }

    Ok(out)
}

fn resolve_patch_path(root: &Path, user_path: &str) -> Result<PathBuf, String> {
    let p = Path::new(user_path);
    if p.is_absolute() {
        return Ok(normalize_path(p));
    }

    let root_abs = normalize_path(root);
    let resolved = normalize_path(&root_abs.join(p));

    if !resolved.starts_with(&root_abs) {
        return Err(format!("Path escapes workspace root: {user_path}"));
    }

    Ok(resolved)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::parse_line_range;

    #[test]
    fn parses_line_ranges() {
        assert_eq!(parse_line_range("L10-L20").unwrap(), (Some(10), Some(20)));
        assert_eq!(parse_line_range("L10-").unwrap(), (Some(10), None));
        assert_eq!(parse_line_range("-L20").unwrap(), (None, Some(20)));
        assert_eq!(parse_line_range("10-20").unwrap(), (Some(10), Some(20)));
    }
}
