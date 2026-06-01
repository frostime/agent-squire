use std::path::{Path, PathBuf};

use regex::Regex;

use super::model::{LinkKind, MdLink, RawLink, TargetType};
use super::sources::display_path;

pub fn resolve_link(raw_link: RawLink, source_file: &Path, workspace: &Path) -> Option<MdLink> {
    let raw = raw_link.raw.trim().to_string();
    if raw.is_empty() {
        return None;
    }

    if raw_link.kind == LinkKind::SiyuanBlock {
        return Some(resolve_siyuan_block(raw_link.line_num, raw_link.kind, &raw));
    }

    if is_url(&raw) {
        return Some(MdLink {
            line_num: raw_link.line_num,
            kind: raw_link.kind,
            raw: raw.clone(),
            target_type: TargetType::Url,
            resolved: Some(raw),
            exists: None,
        });
    }

    if (raw_link.kind == LinkKind::CodeSpan || raw_link.kind == LinkKind::Angle)
        && !looks_like_file_target(&raw)
    {
        return None;
    }

    if raw_link.kind == LinkKind::Wiki || looks_like_file_target(&raw) {
        return Some(resolve_file(raw_link, source_file, workspace));
    }

    Some(MdLink {
        line_num: raw_link.line_num,
        kind: raw_link.kind,
        raw,
        target_type: TargetType::Unknown,
        resolved: None,
        exists: None,
    })
}

fn resolve_siyuan_block(line_num: usize, kind: LinkKind, raw: &str) -> MdLink {
    let id = raw.split_whitespace().next().unwrap_or(raw).to_string();
    MdLink {
        line_num,
        kind,
        raw: raw.to_string(),
        target_type: TargetType::SiyuanBlock,
        resolved: Some(id),
        exists: None,
    }
}

fn resolve_file(raw_link: RawLink, source_file: &Path, workspace: &Path) -> MdLink {
    let raw_for_path = strip_wiki_alias(&raw_link.raw);
    let check_target = strip_fragment_query(raw_for_path);
    let normalized = check_target.replace('\\', "/");
    let candidate = candidate_path(&normalized, source_file, workspace);
    let mut exists = candidate.exists();
    let mut resolved_path = candidate;

    if raw_link.kind == LinkKind::Wiki && !exists && Path::new(&normalized).extension().is_none() {
        let md_candidate = candidate_path(&format!("{normalized}.md"), source_file, workspace);
        if md_candidate.exists() {
            exists = true;
            resolved_path = md_candidate;
        } else {
            resolved_path = md_candidate;
        }
    }

    MdLink {
        line_num: raw_link.line_num,
        kind: raw_link.kind,
        raw: raw_link.raw,
        target_type: TargetType::File,
        resolved: Some(display_path(&resolved_path, workspace)),
        exists: Some(exists),
    }
}

fn candidate_path(target: &str, source_file: &Path, workspace: &Path) -> PathBuf {
    if is_windows_absolute(target) {
        return PathBuf::from(target);
    }

    if let Some(stripped) = target.strip_prefix('/') {
        let workspace_candidate = workspace.join(stripped);
        if workspace_candidate.exists() || !Path::new(target).exists() {
            return workspace_candidate;
        }
        return PathBuf::from(target);
    }

    if target.starts_with("./") || target.starts_with("../") {
        return source_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(target);
    }

    workspace.join(target)
}

fn is_url(target: &str) -> bool {
    target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("siyuan://")
}

fn looks_like_file_target(target: &str) -> bool {
    let target = target.trim();
    if target.starts_with("./")
        || target.starts_with("../")
        || target.starts_with('/')
        || target.starts_with("~/")
        || is_windows_absolute(target)
        || target.contains('/')
        || target.contains('\\')
    {
        return true;
    }

    let Some(ext) = Path::new(strip_fragment_query(target)).extension() else {
        return false;
    };
    matches!(
        ext.to_string_lossy().to_ascii_lowercase().as_str(),
        "md" | "markdown"
            | "txt"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "rs"
            | "toml"
            | "json"
            | "yaml"
            | "yml"
            | "js"
            | "ts"
            | "tsx"
            | "jsx"
            | "py"
            | "html"
            | "css"
    )
}

fn is_windows_absolute(target: &str) -> bool {
    let regex = Regex::new(r"^[A-Za-z]:[/\\]").expect("valid windows path regex");
    regex.is_match(target)
}

fn strip_fragment_query(target: &str) -> &str {
    target.split(['#', '?']).next().unwrap_or(target).trim()
}

fn strip_wiki_alias(target: &str) -> &str {
    target.split('|').next().unwrap_or(target).trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_spans_ignore_plain_words() {
        let raw = RawLink {
            line_num: 1,
            kind: LinkKind::CodeSpan,
            raw: "plain".into(),
        };

        assert!(resolve_link(raw, Path::new("a.md"), Path::new(".")).is_none());
    }
}
