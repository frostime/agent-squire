//! Shared Markdown scanning primitives for builtins.
//!
//! Builtins that walk Markdown documents line-by-line (currently `toc` and
//! `md_links`) share identical fenced-code-block tracking. This module owns
//! that primitive so each consumer only handles the tokens it cares about.

/// Iterate lines of a Markdown document, skipping fenced code blocks.
///
/// Yields `(1-based line number, the original line text)` for every line that
/// is not inside (and not itself) a fenced code block. A fence is opened by a
/// line whose trimmed-start begins with `` ``` `` or `~~~`; it is closed by a
/// line whose trimmed-start begins with the same three-character marker that
/// opened it. Fence lines themselves are skipped (not yielded).
///
/// This consolidates the fence-tracking loop previously duplicated in
/// `toc::parse_headings` and `md_links::parse::parse_links`. The yielded line
/// is the raw line (no trimming); callers apply their own normalization.
pub fn iter_prose_lines(content: &str) -> ProseLines<'_> {
    ProseLines {
        lines: content.lines(),
        line_num: 0,
        in_fence: false,
        fence_marker: "",
    }
}

pub struct ProseLines<'a> {
    lines: std::str::Lines<'a>,
    line_num: usize,
    in_fence: bool,
    fence_marker: &'a str,
}

impl<'a> Iterator for ProseLines<'a> {
    type Item = (usize, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        for line in self.lines.by_ref() {
            self.line_num += 1;
            let stripped = line.trim_start();
            if stripped.starts_with("```") || stripped.starts_with("~~~") {
                let marker = &stripped[..3];
                if !self.in_fence {
                    self.in_fence = true;
                    self.fence_marker = marker;
                } else if marker == self.fence_marker {
                    self.in_fence = false;
                }
                continue;
            }
            if self.in_fence {
                continue;
            }
            return Some((self.line_num, line));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(content: &str) -> Vec<(usize, String)> {
        iter_prose_lines(content)
            .map(|(n, l)| (n, l.to_string()))
            .collect()
    }

    #[test]
    fn yields_all_lines_when_no_fence() {
        assert_eq!(
            collect("a\nb\nc"),
            vec![(1, "a".into()), (2, "b".into()), (3, "c".into())]
        );
    }

    #[test]
    fn fenced_code_blocks_are_skipped() {
        let lines = collect("[ok](a.md)\n```\n[skip](b.md)\n```\n[[Wiki]]");
        assert_eq!(
            lines,
            vec![(1, "[ok](a.md)".into()), (5, "[[Wiki]]".into()),]
        );
    }

    #[test]
    fn tilde_fences_match_only_same_marker() {
        let lines = collect("a\n~~~\n```code```\n~~~\nb");
        assert_eq!(lines, vec![(1, "a".into()), (5, "b".into())]);
    }

    #[test]
    fn mismatched_marker_does_not_close_fence() {
        // ``` opens; ~~~ is inside the block, not a closer; close needs ```
        let lines = collect("a\n```\n~~~\n```\nb");
        assert_eq!(lines, vec![(1, "a".into()), (5, "b".into())]);
    }

    #[test]
    fn indented_fence_is_recognized() {
        let lines = collect("text\n  ```\n  inside\n  ```\nend");
        assert_eq!(lines, vec![(1, "text".into()), (5, "end".into())]);
    }

    #[test]
    fn empty_input_yields_nothing() {
        assert!(collect("").is_empty());
    }
}
