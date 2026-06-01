use regex::Regex;

use super::model::{LinkKind, RawLink};

pub fn parse_links(content: &str) -> Vec<RawLink> {
    let mut links = Vec::new();
    let mut in_fence = false;
    let mut fence_marker = "";

    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        let stripped = line.trim_start();

        if stripped.starts_with("```") || stripped.starts_with("~~~") {
            let marker = &stripped[..3];
            if !in_fence {
                in_fence = true;
                fence_marker = marker;
            } else if marker == fence_marker {
                in_fence = false;
            }
            continue;
        }

        if in_fence {
            continue;
        }

        scan_line(line, line_num, &mut links);
    }

    links
}

fn scan_line(line: &str, line_num: usize, links: &mut Vec<RawLink>) {
    scan_regex(
        line,
        line_num,
        links,
        r"!\[[^\]]*\]\(([^)]+)\)",
        LinkKind::Image,
    );
    scan_regex(
        line,
        line_num,
        links,
        r"(?P<prefix>^|[^!])\[[^\]]+\]\(([^)]+)\)",
        LinkKind::Markdown,
    );
    scan_regex(line, line_num, links, r"\[\[([^\]]+)\]\]", LinkKind::Wiki);
    scan_regex(
        line,
        line_num,
        links,
        r#"\(\(([0-9]{14}-[a-z0-9]+\s+["'][^"']+["'])\)\)"#,
        LinkKind::SiyuanBlock,
    );
    scan_regex(line, line_num, links, r"`([^`]+)`", LinkKind::CodeSpan);
    scan_regex(line, line_num, links, r"<([^>\s]+)>", LinkKind::Angle);
}

fn scan_regex(
    line: &str,
    line_num: usize,
    links: &mut Vec<RawLink>,
    pattern: &str,
    kind: LinkKind,
) {
    let regex = Regex::new(pattern).expect("valid link regex");
    for captures in regex.captures_iter(line) {
        let Some(matched) = captures.get(captures.len() - 1) else {
            continue;
        };
        let raw = matched.as_str().trim();
        if raw.is_empty() {
            continue;
        }
        links.push(RawLink {
            line_num,
            kind: kind.clone(),
            raw: raw.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fenced_code_blocks_are_skipped() {
        let links = parse_links("[ok](a.md)\n```\n[skip](b.md)\n```\n[[Wiki]]");

        assert_eq!(links.len(), 2);
        assert_eq!(links[0].raw, "a.md");
        assert_eq!(links[1].raw, "Wiki");
    }
}
