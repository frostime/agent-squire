//! DSL parser: plain text → [`Spec`].
//!
//! Grammar (one action per spec):
//! ```text
//! file <path>
//! chunk <name> = <N> | <A>-<B>
//! move|copy <region> to <anchor>
//! delete <region>
//! rearrange <names> => <names> [gap=slot|drop|error]
//! ```
//! `<region>` is an inline range (`10-20`) or a declared chunk name.
//! Blank lines and `#` comment lines are ignored.

use std::path::PathBuf;

use crate::builtins::rearrange::model::{
    Action, Anchor, ChunkDef, ErrorCode, Gap, RearrangeError, Region, Result, Spec,
};

pub fn parse(input: &str) -> Result<Spec> {
    let mut file: Option<PathBuf> = None;
    let mut chunks: Vec<ChunkDef> = Vec::new();
    let mut action: Option<Action> = None;

    for raw in input.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix("file ") {
            if file.is_some() {
                return Err(err(
                    ErrorCode::InvalidSpec,
                    "multiple `file` directives; v1 targets one file",
                ));
            }
            file = Some(PathBuf::from(rest.trim()));
        } else if let Some(rest) = line.strip_prefix("chunk ") {
            chunks.push(parse_chunk(rest)?);
        } else if is_action_line(line) {
            if action.is_some() {
                return Err(err(
                    ErrorCode::MultipleActions,
                    "v1 allows exactly one action per spec",
                ));
            }
            action = Some(parse_action(line)?);
        } else {
            return Err(err(
                ErrorCode::InvalidSpec,
                format!("unrecognized line: {line}"),
            ));
        }
    }

    let file = file.ok_or_else(|| err(ErrorCode::InvalidSpec, "missing `file` directive"))?;
    let action = action.ok_or_else(|| err(ErrorCode::InvalidSpec, "no action found"))?;
    Ok(Spec {
        file,
        chunks,
        action,
    })
}

fn is_action_line(line: &str) -> bool {
    ["move ", "copy ", "delete ", "rearrange "]
        .iter()
        .any(|kw| line.starts_with(kw))
}

fn parse_chunk(rest: &str) -> Result<ChunkDef> {
    let (name, range) = rest
        .split_once('=')
        .ok_or_else(|| err(ErrorCode::InvalidSpec, format!("chunk needs `=`: {rest}")))?;
    let (start, end) = parse_range(range.trim())?;
    Ok(ChunkDef {
        name: name.trim().to_string(),
        start,
        end,
    })
}

fn parse_action(line: &str) -> Result<Action> {
    if let Some(rest) = line.strip_prefix("rearrange ") {
        return parse_rearrange(rest);
    }
    if let Some(rest) = line.strip_prefix("delete ") {
        return Ok(Action::Delete {
            src: parse_region(rest.trim())?,
        });
    }
    // move / copy share `<region> to <anchor>`.
    let (kw, rest) = line.split_once(' ').expect("checked prefix");
    let (region, anchor) = rest.split_once(" to ").ok_or_else(|| {
        err(
            ErrorCode::InvalidSpec,
            format!("`{kw}` needs `to <anchor>`: {line}"),
        )
    })?;
    let src = parse_region(region.trim())?;
    let to = parse_anchor(anchor.trim())?;
    Ok(match kw {
        "move" => Action::Move { src, to },
        "copy" => Action::Copy { src, to },
        _ => unreachable!("is_action_line gates the keyword"),
    })
}

fn parse_rearrange(rest: &str) -> Result<Action> {
    let (lists, gap) = match rest.split_once("gap=") {
        Some((head, g)) => (head.trim(), parse_gap(g.trim())?),
        None => (rest.trim(), Gap::Slot),
    };
    let (from_raw, to_raw) = lists.split_once("=>").ok_or_else(|| {
        err(
            ErrorCode::InvalidSpec,
            format!("rearrange needs `=>`: {rest}"),
        )
    })?;
    let from = parse_name_list(from_raw);
    let to = parse_name_list(to_raw);
    if from.is_empty() || to.is_empty() {
        return Err(err(
            ErrorCode::InvalidSpec,
            "rearrange needs chunk names on both sides",
        ));
    }
    Ok(Action::Rearrange { from, to, gap })
}

fn parse_name_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_gap(raw: &str) -> Result<Gap> {
    match raw {
        "slot" => Ok(Gap::Slot),
        "drop" => Ok(Gap::Drop),
        "error" => Ok(Gap::Error),
        other => Err(err(
            ErrorCode::InvalidSpec,
            format!("unknown gap policy: {other}"),
        )),
    }
}

/// An inline `A-B`/`N` parses to a range; anything else is a chunk name.
fn parse_region(raw: &str) -> Result<Region> {
    if raw.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        let (start, end) = parse_range(raw)?;
        Ok(Region::Inline { start, end })
    } else {
        Ok(Region::Named(raw.to_string()))
    }
}

fn parse_anchor(raw: &str) -> Result<Anchor> {
    match raw {
        "start" => Ok(Anchor::Start),
        "end" => Ok(Anchor::End),
        _ => {
            if let Some(n) = raw.strip_prefix("before ") {
                Ok(Anchor::Before(parse_line(n.trim())?))
            } else if let Some(n) = raw.strip_prefix("after ") {
                Ok(Anchor::After(parse_line(n.trim())?))
            } else {
                Err(err(
                    ErrorCode::InvalidSpec,
                    format!("invalid anchor: {raw}"),
                ))
            }
        }
    }
}

fn parse_range(raw: &str) -> Result<(usize, usize)> {
    match raw.split_once('-') {
        Some((a, b)) => {
            let start = parse_line(a.trim())?;
            let end = parse_line(b.trim())?;
            if start > end {
                return Err(err(
                    ErrorCode::InvalidRange,
                    format!("range start after end: {raw}"),
                ));
            }
            Ok((start, end))
        }
        None => {
            let n = parse_line(raw)?;
            Ok((n, n))
        }
    }
}

fn parse_line(raw: &str) -> Result<usize> {
    let n: usize = raw
        .parse()
        .map_err(|_| err(ErrorCode::InvalidRange, format!("not a line number: {raw}")))?;
    if n == 0 {
        return Err(err(ErrorCode::InvalidRange, "line numbers are 1-based"));
    }
    Ok(n)
}

fn err(code: ErrorCode, message: impl Into<String>) -> RearrangeError {
    RearrangeError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_move_inline() {
        let spec = parse("file a.md\nmove 40-90 to after 120").unwrap();
        assert!(matches!(spec.action, Action::Move { .. }));
    }

    #[test]
    fn parses_rearrange_with_gap() {
        let spec =
            parse("file a.md\nchunk A = 1-10\nchunk B = 15-20\nrearrange A, B => B, A gap=drop")
                .unwrap();
        match spec.action {
            Action::Rearrange { from, to, gap } => {
                assert_eq!(from, ["A", "B"]);
                assert_eq!(to, ["B", "A"]);
                assert!(matches!(gap, Gap::Drop));
            }
            _ => panic!("expected rearrange"),
        }
    }

    #[test]
    fn rejects_multiple_actions() {
        let e = parse("file a.md\ndelete 1-2\ndelete 3-4").unwrap_err();
        assert_eq!(e.code, ErrorCode::MultipleActions);
    }

    #[test]
    fn rejects_missing_file() {
        let e = parse("delete 1-2").unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidSpec);
    }

    #[test]
    fn rejects_inverted_range() {
        let e = parse("file a.md\ndelete 20-10").unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidRange);
    }
}
