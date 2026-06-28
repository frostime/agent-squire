//! Planner: [`Spec`] + file contents → new line array, all in original-snapshot
//! coordinates. Three stages run in `execute`: resolve (names/inline → indices,
//! read file), validate (bounds/overlap/set/anchor), materialize (build output).
//!
//! Internally line indices are 0-based half-open where convenient; the DSL's
//! 1-based inclusive ranges are converted at the resolve boundary.

use std::path::Path;

use crate::builtins::rearrange::model::{
    Action, Anchor, ErrorCode, Gap, RearrangeError, Region, Result, Spec,
};
use crate::builtins::rearrange::parser;
use crate::builtins::rearrange::textio::{self, TextFile};

/// What the planner produces for rendering: the resolved facts plus the result.
pub struct Outcome {
    pub file_path: String,
    pub summary: Summary,
    pub original: Vec<String>,
    pub new_lines: Vec<String>,
    pub changed: bool,
}

/// Human-facing description of the action, built during planning.
pub enum Summary {
    Move {
        start: usize,
        end: usize,
        anchor: String,
    },
    Copy {
        start: usize,
        end: usize,
        anchor: String,
    },
    Delete {
        start: usize,
        end: usize,
    },
    Rearrange {
        chunks: Vec<(String, usize, usize)>, // name, 1-based start, end
        from: Vec<String>,
        to: Vec<String>,
        gap: GapReport,
    },
}

pub enum GapReport {
    Slot(Vec<(usize, usize)>), // kept inter-slot gaps, 1-based inclusive
    Dropped(Vec<(usize, usize)>),
    None,
}

/// Parse, plan, and optionally write. Returns the outcome for rendering.
pub fn execute(spec_text: &str, cwd: &Path, write: bool) -> Result<Outcome> {
    let spec = parser::parse(spec_text)?;
    let path = cwd.join(&spec.file);
    if !path.is_file() {
        return Err(err(
            ErrorCode::FileNotFound,
            format!("file not found: {}", spec.file.display()),
        ));
    }
    let file = textio::read_file(&path).map_err(|e| err(ErrorCode::InvalidSpec, e.to_string()))?;

    let (new_lines, summary) = plan(&spec, &file)?;
    let changed = new_lines != file.lines;

    if write && changed {
        let bytes = file.render(&new_lines);
        textio::write_file(&path, &bytes)
            .map_err(|e| err(ErrorCode::InvalidSpec, e.to_string()))?;
    }

    Ok(Outcome {
        file_path: spec.file.display().to_string().replace('\\', "/"),
        summary,
        original: file.lines,
        new_lines,
        changed,
    })
}

fn plan(spec: &Spec, file: &TextFile) -> Result<(Vec<String>, Summary)> {
    let len = file.lines.len();
    match &spec.action {
        Action::Move { src, to } => {
            let (a, b) = resolve_region(src, spec, len)?;
            let ins = resolve_anchor(to, len)?;
            reject_anchor_inside(ins, a, b)?;
            let block = file.lines[a - 1..b].to_vec();
            let out = splice(&file.lines, ins, &block, Some((a, b)));
            Ok((
                out,
                Summary::Move {
                    start: a,
                    end: b,
                    anchor: anchor_label(to),
                },
            ))
        }
        Action::Copy { src, to } => {
            let (a, b) = resolve_region(src, spec, len)?;
            let ins = resolve_anchor(to, len)?;
            let block = file.lines[a - 1..b].to_vec();
            let out = splice(&file.lines, ins, &block, None);
            Ok((
                out,
                Summary::Copy {
                    start: a,
                    end: b,
                    anchor: anchor_label(to),
                },
            ))
        }
        Action::Delete { src } => {
            let (a, b) = resolve_region(src, spec, len)?;
            let out = splice(&file.lines, 0, &[], Some((a, b)));
            Ok((out, Summary::Delete { start: a, end: b }))
        }
        Action::Rearrange { from, to, gap } => plan_rearrange(spec, file, from, to, gap),
    }
}

/// Move/copy/delete share one rebuild: walk the original lines, insert `block`
/// at index `ins` (0-based, "before line ins+1"), and skip `cut` if removing.
fn splice(
    lines: &[String],
    ins: usize,
    block: &[String],
    cut: Option<(usize, usize)>,
) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len() + block.len());
    for i in 0..=lines.len() {
        if i == ins {
            out.extend_from_slice(block);
        }
        if i < lines.len() {
            if let Some((a, b)) = cut {
                let line = i + 1;
                if line >= a && line <= b {
                    continue;
                }
            }
            out.push(lines[i].clone());
        }
    }
    out
}

fn plan_rearrange(
    spec: &Spec,
    file: &TextFile,
    from: &[String],
    to: &[String],
    gap: &Gap,
) -> Result<(Vec<String>, Summary)> {
    // Physical slots are the declared chunks ordered by their start line.
    let mut slots: Vec<(String, usize, usize)> = from
        .iter()
        .map(|name| {
            let def = spec
                .chunks
                .iter()
                .find(|c| &c.name == name)
                .ok_or_else(|| err(ErrorCode::UnknownChunk, format!("unknown chunk: {name}")))?;
            Ok((name.clone(), def.start, def.end))
        })
        .collect::<Result<Vec<_>>>()?;
    slots.sort_by_key(|(_, start, _)| *start);

    validate_set(from, to)?;
    validate_slots(&slots, file.lines.len())?;

    // Capture each chunk's content and the gaps that sit between adjacent slots.
    let content = |name: &str| -> Vec<String> {
        let (_, s, e) = slots
            .iter()
            .find(|(n, _, _)| n == name)
            .expect("set validated");
        file.lines[s - 1..*e].to_vec()
    };
    // Gaps are positional: gaps[i] sits between slot i and slot i+1 (empty when
    // the slots are adjacent), so it stays aligned with the slot it follows.
    let mut gaps: Vec<(usize, usize, Vec<String>)> = Vec::new(); // 1-based start,end inclusive; empty range = (0,0)
    for pair in slots.windows(2) {
        let (_, _, prev_end) = &pair[0];
        let (_, next_start, _) = &pair[1];
        if next_start - prev_end > 1 {
            let gs = prev_end + 1;
            let ge = next_start - 1;
            gaps.push((gs, ge, file.lines[gs - 1..ge].to_vec()));
        } else {
            gaps.push((0, 0, Vec::new()));
        }
    }

    if matches!(gap, Gap::Error) && gaps.iter().any(|(_, _, g)| !g.is_empty()) {
        return Err(err(
            ErrorCode::NonEmptyGap,
            "non-empty gap between chunks (gap=error)",
        ));
    }

    // Rebuild the span: slot i receives the content named by `to[i]`, with the
    // original gaps kept (slot) or omitted (drop) between slots.
    let keep_gaps = matches!(gap, Gap::Slot);
    let mut rebuilt = Vec::new();
    for (i, name) in to.iter().enumerate() {
        rebuilt.extend(content(name));
        if keep_gaps {
            if let Some((_, _, g)) = gaps.get(i) {
                rebuilt.extend(g.clone());
            }
        }
    }

    let span_start = slots.first().expect("non-empty").1;
    let span_end = slots.last().expect("non-empty").2;
    let mut out = file.lines[..span_start - 1].to_vec();
    out.extend(rebuilt);
    out.extend_from_slice(&file.lines[span_end..]);

    let gap_report = match gap {
        Gap::Slot => GapReport::Slot(non_empty_gaps(&gaps)),
        Gap::Drop => GapReport::Dropped(non_empty_gaps(&gaps)),
        Gap::Error => GapReport::None,
    };
    let summary = Summary::Rearrange {
        chunks: slots,
        from: from.to_vec(),
        to: to.to_vec(),
        gap: gap_report,
    };
    Ok((out, summary))
}

fn resolve_region(region: &Region, spec: &Spec, len: usize) -> Result<(usize, usize)> {
    let (a, b) = match region {
        Region::Inline { start, end } => (*start, *end),
        Region::Named(name) => {
            let def = spec
                .chunks
                .iter()
                .find(|c| &c.name == name)
                .ok_or_else(|| err(ErrorCode::UnknownChunk, format!("unknown chunk: {name}")))?;
            (def.start, def.end)
        }
    };
    if b > len {
        return Err(err(
            ErrorCode::RangeOutOfBounds,
            format!("range {a}-{b} exceeds file ({len} lines)"),
        ));
    }
    Ok((a, b))
}

/// Anchor → 0-based insertion index ("before line index+1").
fn resolve_anchor(anchor: &Anchor, len: usize) -> Result<usize> {
    let n = match anchor {
        Anchor::Start => return Ok(0),
        Anchor::End => return Ok(len),
        Anchor::Before(n) => *n,
        Anchor::After(n) => *n,
    };
    if n < 1 || n > len {
        return Err(err(
            ErrorCode::AnchorOutOfBounds,
            format!("anchor line {n} out of bounds (1-{len})"),
        ));
    }
    Ok(match anchor {
        Anchor::Before(n) => n - 1,
        Anchor::After(n) => *n,
        _ => unreachable!(),
    })
}

/// A move whose anchor lands strictly inside the moved block is ambiguous.
/// Landing on a boundary (before start / after end) is a no-op and allowed.
fn reject_anchor_inside(ins: usize, a: usize, b: usize) -> Result<()> {
    if ins > a - 1 && ins < b {
        return Err(err(
            ErrorCode::AnchorInsideMovedChunk,
            format!("anchor falls inside moved range {a}-{b}"),
        ));
    }
    Ok(())
}

fn validate_set(from: &[String], to: &[String]) -> Result<()> {
    let mut f = from.to_vec();
    let mut t = to.to_vec();
    f.sort();
    t.sort();
    // Duplicate names make slot assignment ambiguous; `from`/`to` are sets.
    if f.windows(2).any(|w| w[0] == w[1]) || t.windows(2).any(|w| w[0] == w[1]) {
        return Err(err(
            ErrorCode::RearrangeSetMismatch,
            "rearrange chunk names must be unique",
        ));
    }
    if f != t {
        return Err(err(
            ErrorCode::RearrangeSetMismatch,
            "rearrange `from` and `to` must contain the same chunk set",
        ));
    }
    Ok(())
}

fn validate_slots(slots: &[(String, usize, usize)], len: usize) -> Result<()> {
    for (name, s, e) in slots {
        if *e > len {
            return Err(err(
                ErrorCode::RangeOutOfBounds,
                format!("chunk {name} ({s}-{e}) exceeds file ({len} lines)"),
            ));
        }
    }
    for pair in slots.windows(2) {
        let (an, _, ae) = &pair[0];
        let (bn, bs, _) = &pair[1];
        if bs <= ae {
            return Err(err(
                ErrorCode::OverlappingChunks,
                format!("chunks {an} and {bn} overlap"),
            ));
        }
    }
    Ok(())
}

fn anchor_label(anchor: &Anchor) -> String {
    match anchor {
        Anchor::Start => "start".into(),
        Anchor::End => "end".into(),
        Anchor::Before(n) => format!("before {n}"),
        Anchor::After(n) => format!("after {n}"),
    }
}

/// Drop empty positional placeholders for reporting; only real gaps are shown.
fn non_empty_gaps(gaps: &[(usize, usize, Vec<String>)]) -> Vec<(usize, usize)> {
    gaps.iter()
        .filter(|(_, _, g)| !g.is_empty())
        .map(|(s, e, _)| (*s, *e))
        .collect()
}

fn err(code: ErrorCode, message: impl Into<String>) -> RearrangeError {
    RearrangeError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::rearrange::textio::{Encoding, Newline};

    fn text(lines: &[&str]) -> TextFile {
        TextFile {
            lines: lines.iter().map(|s| s.to_string()).collect(),
            encoding: Encoding::Utf8,
            newline: Newline::Lf,
            trailing_newline: true,
        }
    }

    fn plan_err(spec: &Spec, file: &TextFile) -> ErrorCode {
        plan(spec, file)
            .err()
            .expect("expected planning to fail")
            .code
    }

    #[test]
    fn rearrange_keeps_gaps_in_slots() {
        // A=1, hidden=2, B=3, C=4, hidden=5, D=6  → reorder to B,D,C,A
        let file = text(&["A", "h1", "B", "C", "h2", "D"]);
        let spec = parser::parse(
            "file a.md\nchunk A = 1-1\nchunk B = 3-3\nchunk C = 4-4\nchunk D = 6-6\nrearrange A, B, C, D => B, D, C, A",
        )
        .unwrap();
        let (out, _) = plan(&spec, &file).unwrap();
        assert_eq!(out, ["B", "h1", "D", "C", "h2", "A"]);
    }

    #[test]
    fn move_block_after_line() {
        let file = text(&["1", "2", "3", "4", "5"]);
        let spec = parser::parse("file a.md\nmove 1-2 to after 4").unwrap();
        let (out, _) = plan(&spec, &file).unwrap();
        assert_eq!(out, ["3", "4", "1", "2", "5"]);
    }

    #[test]
    fn overlapping_chunks_rejected() {
        let file = text(&["1", "2", "3", "4"]);
        let spec = parser::parse("file a.md\nchunk A = 1-2\nchunk B = 2-3\nrearrange A, B => B, A")
            .unwrap();
        assert_eq!(plan_err(&spec, &file), ErrorCode::OverlappingChunks);
    }

    #[test]
    fn anchor_inside_moved_rejected() {
        let file = text(&["1", "2", "3", "4", "5"]);
        let spec = parser::parse("file a.md\nmove 1-3 to after 2").unwrap();
        assert_eq!(plan_err(&spec, &file), ErrorCode::AnchorInsideMovedChunk);
    }

    #[test]
    fn gap_drop_removes_hidden() {
        let file = text(&["A", "h", "B"]);
        let spec = parser::parse(
            "file a.md\nchunk A = 1-1\nchunk B = 3-3\nrearrange A, B => B, A gap=drop",
        )
        .unwrap();
        let (out, _) = plan(&spec, &file).unwrap();
        assert_eq!(out, ["B", "A"]);
    }
}
