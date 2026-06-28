//! Render planner [`Outcome`]s: compact (human), json (agent), and the unified
//! diff shared by both. Errors carry a structured code in JSON `meta`.

use crate::builtins::rearrange::model::RearrangeError;
use crate::builtins::rearrange::plan::{GapReport, Outcome, Summary};
use crate::runtime::output::{Envelope, PrintMode, print_json};

pub fn render(outcome: &Outcome, written: bool, mode: PrintMode) {
    match mode {
        PrintMode::Json => render_json(outcome, written),
        _ => render_compact(outcome, written),
    }
}

pub fn render_error(err: &RearrangeError, mode: PrintMode) {
    match mode {
        PrintMode::Json => {
            let payload = Envelope {
                ok: false,
                command: "rearrange",
                data: serde_json::Value::Null,
                warnings: vec![],
                meta: serde_json::json!({
                    "error_code": err.code.as_str(),
                    "message": err.message,
                }),
            };
            let _ = print_json(&payload);
        }
        _ => eprintln!("error: {err}"),
    }
}

fn render_compact(outcome: &Outcome, written: bool) {
    let state = if written && outcome.changed {
        "written"
    } else if written {
        "no-op"
    } else {
        "dry-run"
    };
    println!("rearrange {}  ({state})", outcome.file_path);
    println!();

    match &outcome.summary {
        Summary::Move { start, end, anchor } => {
            println!(
                "  action  move {start}-{end} ({} lines) -> {anchor}",
                end - start + 1
            );
        }
        Summary::Copy { start, end, anchor } => {
            println!(
                "  action  copy {start}-{end} ({} lines) -> {anchor}",
                end - start + 1
            );
        }
        Summary::Delete { start, end } => {
            println!("  action  delete {start}-{end} ({} lines)", end - start + 1);
        }
        Summary::Rearrange {
            chunks,
            from,
            to,
            gap,
        } => {
            for (name, s, e) in chunks {
                println!("  chunk {name}  {s}-{e}  ({} lines)", e - s + 1);
            }
            println!(
                "  action  rearrange {} => {}",
                from.join(", "),
                to.join(", ")
            );
            render_gap(gap);
        }
    }
    println!();

    print_diff(&outcome.file_path, &outcome.original, &outcome.new_lines);
    println!();

    if !outcome.changed {
        println!("No change.");
    } else if written {
        println!("{} modified", outcome.file_path);
    } else {
        println!("No file written. Pass --yes to apply.");
    }
}

fn render_gap(gap: &GapReport) {
    match gap {
        GapReport::Slot(gaps) if !gaps.is_empty() => {
            let parts: Vec<String> = gaps.iter().map(|(s, e)| format!("{s}-{e} kept")).collect();
            println!("  gaps    {}", parts.join("   "));
        }
        GapReport::Dropped(gaps) if !gaps.is_empty() => {
            for (s, e) in gaps {
                println!("  dropped {s}-{e} ({} lines)", e - s + 1);
            }
        }
        _ => {}
    }
}

fn render_json(outcome: &Outcome, written: bool) {
    let mut data = serde_json::json!({
        "written": written && outcome.changed,
        "changed": outcome.changed,
        "file": outcome.file_path,
        "action": action_json(&outcome.summary),
        "diff": diff_text(&outcome.file_path, &outcome.original, &outcome.new_lines),
    });
    if let Some(chunks) = chunks_json(&outcome.summary) {
        data["chunks"] = chunks;
    }
    let payload = Envelope {
        ok: true,
        command: "rearrange",
        data,
        warnings: vec![],
        meta: serde_json::json!({}),
    };
    let _ = print_json(&payload);
}

/// Structured action descriptor (BC-5). Shape varies by action type.
fn action_json(summary: &Summary) -> serde_json::Value {
    match summary {
        Summary::Move { start, end, anchor } => serde_json::json!({
            "type": "move", "range": format!("{start}-{end}"), "anchor": anchor,
        }),
        Summary::Copy { start, end, anchor } => serde_json::json!({
            "type": "copy", "range": format!("{start}-{end}"), "anchor": anchor,
        }),
        Summary::Delete { start, end } => serde_json::json!({
            "type": "delete", "range": format!("{start}-{end}"),
        }),
        Summary::Rearrange { from, to, gap, .. } => serde_json::json!({
            "type": "rearrange", "from": from, "to": to, "gap": gap_name(gap),
        }),
    }
}

/// Declared chunks keyed by name; present only for `rearrange` (BC-5).
fn chunks_json(summary: &Summary) -> Option<serde_json::Value> {
    let Summary::Rearrange { chunks, .. } = summary else {
        return None;
    };
    let map: serde_json::Map<String, serde_json::Value> = chunks
        .iter()
        .map(|(name, s, e)| {
            (
                name.clone(),
                serde_json::json!({ "range": format!("{s}-{e}"), "lines": e - s + 1 }),
            )
        })
        .collect();
    Some(serde_json::Value::Object(map))
}

fn gap_name(gap: &GapReport) -> &'static str {
    match gap {
        GapReport::Slot(_) => "slot",
        GapReport::Dropped(_) => "drop",
        GapReport::None => "error",
    }
}

/// A minimal whole-file unified diff: old block removed, new block added. This
/// favors clarity for reordered content over a minimal line-level edit script.
fn print_diff(path: &str, old: &[String], new: &[String]) {
    print!("{}", diff_text(path, old, new));
}

fn diff_text(path: &str, old: &[String], new: &[String]) -> String {
    if old == new {
        return String::new();
    }
    let mut out = String::new();
    out.push_str(&format!("--- a/{path}\n+++ b/{path}\n"));
    out.push_str(&format!("@@ -1,{} +1,{} @@\n", old.len(), new.len()));
    for line in old {
        out.push_str(&format!("-{line}\n"));
    }
    for line in new {
        out.push_str(&format!("+{line}\n"));
    }
    out
}
