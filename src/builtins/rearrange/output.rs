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
    let payload = Envelope {
        ok: true,
        command: "rearrange",
        data: serde_json::json!({
            "written": written && outcome.changed,
            "changed": outcome.changed,
            "file": outcome.file_path,
            "diff": diff_text(&outcome.file_path, &outcome.original, &outcome.new_lines),
        }),
        warnings: vec![],
        meta: serde_json::json!({}),
    };
    let _ = print_json(&payload);
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
