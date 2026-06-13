use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::model::{LineRange, Prefix, Source};

pub fn parse_source(input: &str, cwd: &Path) -> Result<Source> {
    let input = input.trim();
    if input.is_empty() {
        bail!("No source specified");
    }

    if let Some((head, tail)) = input.split_once(':')
        && let Some(prefix) = Prefix::parse(head)
    {
        return parse_prefixed(prefix, tail.trim_start(), cwd);
    }

    parse_auto(input, cwd)
}

pub fn selector_prefix(input: &str) -> Option<Prefix> {
    let trimmed = input.trim();
    let prefix = trimmed.strip_suffix(':')?;
    let parsed = Prefix::parse(prefix)?;
    matches!(
        parsed,
        Prefix::File | Prefix::Dir | Prefix::Tree | Prefix::Glob
    )
    .then_some(parsed)
}

pub fn command_body_prefix(input: &str) -> bool {
    matches!(input.trim().strip_suffix(':'), Some(prefix) if Prefix::parse(prefix) == Some(Prefix::Cmd))
}

fn parse_prefixed(prefix: Prefix, content: &str, cwd: &Path) -> Result<Source> {
    if content.is_empty() {
        bail!("No source specified");
    }
    Ok(match prefix {
        Prefix::File => {
            let (path, range) = parse_file_content(content)?;
            Source::File { path, range }
        }
        Prefix::Dir => Source::Dir {
            path: PathBuf::from(content),
        },
        Prefix::Tree => Source::Tree {
            path: PathBuf::from(content),
        },
        Prefix::Glob => Source::Glob {
            pattern: content.to_string(),
        },
        Prefix::Cmd => Source::Command {
            command: content.to_string(),
        },
    })
    .and_then(|source| validate_auto_detected(source, cwd))
}

fn parse_auto(input: &str, cwd: &Path) -> Result<Source> {
    if input.ends_with('/') || input.ends_with('\\') {
        return validate_auto_detected(
            Source::Dir {
                path: PathBuf::from(input),
            },
            cwd,
        );
    }
    if has_glob_magic(input) {
        return Ok(Source::Glob {
            pattern: input.to_string(),
        });
    }

    let path = PathBuf::from(input);
    let resolved = resolve_path(cwd, &path);
    if resolved.is_file() {
        return Ok(Source::File { path, range: None });
    }
    if resolved.is_dir() {
        return Ok(Source::Dir { path });
    }
    bail!("unknown source type, use prefix syntax");
}

fn validate_auto_detected(source: Source, cwd: &Path) -> Result<Source> {
    match &source {
        Source::File { path, .. } => {
            if !resolve_path(cwd, path).is_file() {
                bail!("File not found: {}", path.display());
            }
        }
        Source::Dir { path } | Source::Tree { path } => {
            if !resolve_path(cwd, path).is_dir() {
                bail!("Directory not found: {}", path.display());
            }
        }
        Source::Glob { .. } | Source::SelectedGlob { .. } | Source::Command { .. } => {}
    }
    Ok(source)
}

pub fn parse_file_content(content: &str) -> Result<(PathBuf, Option<LineRange>)> {
    if let Some((path, range_text)) = split_range_suffix(content) {
        let (start, end) = range_text
            .split_once('-')
            .ok_or_else(|| anyhow::anyhow!("Invalid range: {range_text}. Use format: start-end"))?;
        let start = start
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("Invalid range: {range_text}. Use format: start-end"))?;
        let end = end
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("Invalid range: {range_text}. Use format: start-end"))?;
        return Ok((PathBuf::from(path), Some(LineRange::new(start, end)?)));
    }
    Ok((PathBuf::from(content), None))
}

fn split_range_suffix(content: &str) -> Option<(&str, &str)> {
    let (path, range) = content.rsplit_once(':')?;
    let (start, end) = range.split_once('-')?;
    if start.is_empty() || end.is_empty() {
        return None;
    }
    if start.chars().all(|c| c.is_ascii_digit()) && end.chars().all(|c| c.is_ascii_digit()) {
        Some((path, range))
    } else {
        None
    }
}

fn has_glob_magic(input: &str) -> bool {
    input.contains('*') || input.contains('?') || input.contains('[') || input.contains(']')
}

fn resolve_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_prefix_and_trims_leading_space() {
        let parsed = parse_source("cmd: git status:short", Path::new(".")).unwrap();
        assert_eq!(
            parsed,
            Source::Command {
                command: "git status:short".into()
            }
        );
    }

    #[test]
    fn parses_windows_style_colons_before_range() {
        let (path, range) = parse_file_content(r"C:\Users\a.rs:10-20").unwrap();
        assert_eq!(path, PathBuf::from(r"C:\Users\a.rs"));
        assert_eq!(range, Some(LineRange { start: 10, end: 20 }));
    }

    #[test]
    fn rejects_invalid_range_order() {
        let err = parse_file_content("main.rs:10-5").unwrap_err();
        assert!(err.to_string().contains("end must be >= start"));
    }

    #[test]
    fn recognizes_selector_prefixes() {
        assert_eq!(selector_prefix("file:"), Some(Prefix::File));
        assert_eq!(selector_prefix("cmd:"), None);
        assert!(command_body_prefix("cmd:"));
    }
}
