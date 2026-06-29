//! Parser for the `rearrange` state-transition DSL.
//!
//! This parser deliberately keeps syntax work separate from file-system and
//! provenance validation. It produces an AST with source line numbers and raw
//! range literals; the planner owns path identity and semantic invariants.

use crate::builtins::rearrange::ast::{
    AfterItem, ArrangeAst, BeforeItem, FileState, RangeEnd, RangeExpr, ShareAst, ShareItemAst,
    SpecAst,
};
use crate::builtins::rearrange::error::{ErrorCode, RearrangeError, Result};

pub fn parse(input: &str) -> Result<SpecAst> {
    let mut parser = Parser::new(input);
    parser.parse()
}

struct Parser<'a> {
    lines: Vec<(usize, &'a str)>,
    index: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        let lines = input
            .lines()
            .enumerate()
            .map(|(i, line)| (i + 1, line))
            .collect();
        Self { lines, index: 0 }
    }

    fn parse(&mut self) -> Result<SpecAst> {
        let mut shares = Vec::new();
        let mut arranges = Vec::new();

        while let Some((line_no, line)) = self.next_non_blank() {
            if let Some(rest) = line.strip_prefix("share ") {
                shares.push(self.parse_share(line_no, rest)?);
            } else if let Some(rest) = line.strip_prefix("arrange ") {
                arranges.push(self.parse_arrange(line_no, rest)?);
            } else {
                return Err(err_at(
                    ErrorCode::InvalidSpec,
                    line_no,
                    format!("expected `share` or `arrange`, got: {line}"),
                ));
            }
        }

        if shares.is_empty() && arranges.is_empty() {
            return Err(err(
                ErrorCode::InvalidSpec,
                "no share or arrange blocks found",
            ));
        }
        Ok(SpecAst { shares, arranges })
    }

    fn parse_share(&mut self, line: usize, rest: &str) -> Result<ShareAst> {
        let (slug, path) = parse_slug_path(rest, line, "share")?;
        let mut items = Vec::new();

        loop {
            let Some((line_no, raw)) = self.next_non_blank() else {
                return Err(err_at(
                    ErrorCode::InvalidSpec,
                    line,
                    "unterminated share block",
                ));
            };
            if raw == "end share" {
                break;
            }
            if raw.starts_with("share ") || raw.starts_with("arrange ") {
                return Err(err_at(
                    ErrorCode::InvalidSpec,
                    line_no,
                    "nested blocks are not allowed",
                ));
            }
            let (name, range) = parse_name_range(raw, line_no)?;
            items.push(ShareItemAst {
                name,
                range,
                line: line_no,
            });
        }

        if items.is_empty() {
            return Err(err_at(
                ErrorCode::InvalidSpec,
                line,
                "share must declare at least one item",
            ));
        }
        Ok(ShareAst {
            slug,
            path,
            items,
            line,
        })
    }

    fn parse_arrange(&mut self, line: usize, rest: &str) -> Result<ArrangeAst> {
        let (slug, path) = parse_optional_slug_path(rest, line)?;
        let (before_line, before_raw) = self
            .next_non_blank()
            .ok_or_else(|| err_at(ErrorCode::InvalidSpec, line, "unterminated arrange block"))?;
        let Some(before_rest) = before_raw.strip_prefix("before ") else {
            return Err(err_at(
                ErrorCode::InvalidSpec,
                before_line,
                "arrange block must declare `before` before `after`",
            ));
        };
        let before = parse_before_state(before_rest.trim(), before_line)?;

        let (after_line, after_raw) = self
            .next_non_blank()
            .ok_or_else(|| err_at(ErrorCode::InvalidSpec, line, "unterminated arrange block"))?;
        let Some(after_rest) = after_raw.strip_prefix("after ") else {
            return Err(err_at(
                ErrorCode::InvalidSpec,
                after_line,
                "arrange block must declare `after` after `before`",
            ));
        };
        let after = parse_after_state(after_rest.trim(), after_line)?;

        let (end_line, end_raw) = self
            .next_non_blank()
            .ok_or_else(|| err_at(ErrorCode::InvalidSpec, line, "unterminated arrange block"))?;
        if end_raw != "end arrange" {
            return Err(err_at(
                ErrorCode::InvalidSpec,
                end_line,
                format!("expected `end arrange`, got: {end_raw}"),
            ));
        }

        Ok(ArrangeAst {
            slug,
            path,
            before,
            after,
            line,
        })
    }

    fn next_non_blank(&mut self) -> Option<(usize, &'a str)> {
        while self.index < self.lines.len() {
            let (line_no, raw) = self.lines[self.index];
            self.index += 1;
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            return Some((line_no, trimmed));
        }
        None
    }
}

fn parse_slug_path(rest: &str, line: usize, kind: &str) -> Result<(String, String)> {
    let (slug, path) = split_spaced_equals(rest, line, || {
        format!("{kind} block opener must be `{kind} <slug> = <file>`")
    })?;
    let slug = slug.trim();
    validate_ident(slug, line)?;
    let path = path.trim();
    if path.is_empty() {
        return Err(err_at(
            ErrorCode::InvalidSpec,
            line,
            "path must not be empty",
        ));
    }
    Ok((slug.to_string(), path.to_string()))
}

fn parse_optional_slug_path(rest: &str, line: usize) -> Result<(Option<String>, String)> {
    if let Some((slug, path)) = rest.split_once(" = ") {
        let slug = slug.trim();
        validate_ident(slug, line)?;
        let path = path.trim();
        if path.is_empty() {
            return Err(err_at(
                ErrorCode::InvalidSpec,
                line,
                "path must not be empty",
            ));
        }
        Ok((Some(slug.to_string()), path.to_string()))
    } else {
        if rest.contains('=') {
            return Err(err_at(
                ErrorCode::InvalidSpec,
                line,
                "ambiguous arrange opener; use `arrange <slug> = <file>` or a slugged arrange for paths containing `=`",
            ));
        }
        let path = rest.trim();
        if path.is_empty() {
            return Err(err_at(
                ErrorCode::InvalidSpec,
                line,
                "path must not be empty",
            ));
        }
        Ok((None, path.to_string()))
    }
}

fn parse_before_state(raw: &str, line: usize) -> Result<FileState<BeforeItem>> {
    parse_file_state(raw, line, parse_before_item)
}

fn parse_after_state(raw: &str, line: usize) -> Result<FileState<AfterItem>> {
    parse_file_state(raw, line, parse_after_item)
}

fn parse_file_state<T>(
    raw: &str,
    line: usize,
    parse_item: fn(&str, usize) -> Result<T>,
) -> Result<FileState<T>> {
    match raw {
        "<missing>" => Ok(FileState::Missing),
        "<empty>" => Ok(FileState::Empty),
        _ => {
            let parts = raw.split(',').map(str::trim).collect::<Vec<_>>();
            if parts.iter().any(|item| item.is_empty()) {
                return Err(err_at(
                    ErrorCode::InvalidSpec,
                    line,
                    "sequence contains an empty item",
                ));
            }
            let items = parts
                .into_iter()
                .map(|item| parse_item(item, line))
                .collect::<Result<Vec<_>>>()?;
            if items.is_empty() {
                return Err(err_at(
                    ErrorCode::InvalidSpec,
                    line,
                    "sequence must not be empty",
                ));
            }
            Ok(FileState::Sequence(items))
        }
    }
}

fn parse_before_item(raw: &str, line: usize) -> Result<BeforeItem> {
    if let Some(name) = parse_gap(raw) {
        validate_ident(name, line)?;
        return Ok(BeforeItem::Gap {
            name: name.to_string(),
            line,
        });
    }
    if let Some((name, range)) = raw.split_once(" = ") {
        let name = name.trim();
        validate_ident(name, line)?;
        return Ok(BeforeItem::Named {
            name: name.to_string(),
            range: parse_range(range.trim(), line)?,
            line,
        });
    }
    if raw.contains('=') {
        return Err(err_at(
            ErrorCode::InvalidSpec,
            line,
            format!("expected `<name> = <range>`, got: {raw}"),
        ));
    }
    Ok(BeforeItem::Anonymous {
        range: parse_range(raw, line)?,
        line,
    })
}

fn parse_after_item(raw: &str, line: usize) -> Result<AfterItem> {
    if let Some(name) = parse_gap(raw) {
        validate_ident(name, line)?;
        return Ok(AfterItem::Gap {
            name: name.to_string(),
            line,
        });
    }
    if let Some((slug, name)) = raw.split_once("::") {
        validate_ident(slug.trim(), line)?;
        validate_ident(name.trim(), line)?;
        return Ok(AfterItem::External {
            slug: slug.trim().to_string(),
            name: name.trim().to_string(),
            line,
        });
    }
    if raw.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Ok(AfterItem::Anonymous {
            range: parse_range(raw, line)?,
            line,
        });
    }
    validate_ident(raw, line)?;
    Ok(AfterItem::Local {
        name: raw.to_string(),
        line,
    })
}

fn parse_gap(raw: &str) -> Option<&str> {
    raw.strip_prefix("<gap:")?.strip_suffix('>')
}

fn parse_name_range(raw: &str, line: usize) -> Result<(String, RangeExpr)> {
    let (name, range) = split_spaced_equals(raw, line, || {
        format!("expected `<name> = <range>`, got: {raw}")
    })?;
    let name = name.trim();
    validate_ident(name, line)?;
    Ok((name.to_string(), parse_range(range.trim(), line)?))
}

fn split_spaced_equals(
    raw: &str,
    line: usize,
    message: impl FnOnce() -> String,
) -> Result<(&str, &str)> {
    raw.split_once(" = ")
        .ok_or_else(|| err_at(ErrorCode::InvalidSpec, line, message()))
}

fn parse_range(raw: &str, line: usize) -> Result<RangeExpr> {
    if raw.is_empty() {
        return Err(err_at(ErrorCode::InvalidRange, line, "empty range"));
    }
    if let Some((start, end)) = raw.split_once('-') {
        let start = parse_line_number(start.trim(), line)?;
        let end = if end.trim() == "end" {
            RangeEnd::End
        } else {
            let end = parse_line_number(end.trim(), line)?;
            if end < start {
                return Err(err_at(
                    ErrorCode::InvalidRange,
                    line,
                    format!("range start after end: {raw}"),
                ));
            }
            RangeEnd::Line(end)
        };
        Ok(RangeExpr {
            raw: raw.to_string(),
            start,
            end,
        })
    } else {
        let n = parse_line_number(raw, line)?;
        Ok(RangeExpr {
            raw: raw.to_string(),
            start: n,
            end: RangeEnd::Line(n),
        })
    }
}

fn parse_line_number(raw: &str, line: usize) -> Result<usize> {
    let n = raw.parse::<usize>().map_err(|_| {
        err_at(
            ErrorCode::InvalidRange,
            line,
            format!("not a line number: {raw}"),
        )
    })?;
    if n == 0 {
        return Err(err_at(
            ErrorCode::InvalidRange,
            line,
            "line numbers are 1-based",
        ));
    }
    Ok(n)
}

const KEYWORDS: &[&str] = &[
    "share", "arrange", "before", "after", "end", "missing", "empty", "gap",
];

fn validate_ident(name: &str, line: usize) -> Result<()> {
    let mut chars = name.chars();
    let valid_head = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    let valid_tail = chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid_head || !valid_tail || KEYWORDS.contains(&name) {
        return Err(err_at(
            ErrorCode::InvalidName,
            line,
            format!("invalid identifier `{name}`"),
        ));
    }
    Ok(())
}

fn err(code: ErrorCode, message: impl Into<String>) -> RearrangeError {
    RearrangeError::new(code, message)
}

fn err_at(code: ErrorCode, line: usize, message: impl Into<String>) -> RearrangeError {
    RearrangeError::at_line(code, line, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_share_and_arrange() {
        let spec = parse(
            "share tpl = snippets/header.rs\n  header = 1-end\nend share\n\narrange main = src/foo.rs\n  before head = 1-10, tail = 11-end\n  after  tpl::header, tail, head\nend arrange",
        )
        .unwrap();
        assert_eq!(spec.shares.len(), 1);
        assert_eq!(spec.arranges.len(), 1);
        assert_eq!(spec.arranges[0].slug.as_deref(), Some("main"));
    }

    #[test]
    fn rejects_after_before_before() {
        let e =
            parse("arrange a.md\n  after <empty>\n  before <missing>\nend arrange").unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidSpec);
    }

    #[test]
    fn rejects_bad_identifier() {
        let e = parse("share 1tpl = a.md\n  header = 1-end\nend share").unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidName);
    }

    #[test]
    fn rejects_unspaced_structural_equals() {
        let e =
            parse("arrange main=foo.rs\n  before A = 1-end\n  after A\nend arrange").unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidSpec);

        let e = parse("share tpl = a.md\n  header=1-end\nend share").unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidSpec);
    }

    #[test]
    fn parses_numeric_eof_guard() {
        let spec = parse("arrange a.md\n  before body = 1-3\n  after body\nend arrange").unwrap();
        let FileState::Sequence(items) = &spec.arranges[0].before else {
            panic!("expected sequence");
        };
        let BeforeItem::Named { range, .. } = &items[0] else {
            panic!("expected named range");
        };
        assert_eq!(range.raw, "1-3");
    }
}
