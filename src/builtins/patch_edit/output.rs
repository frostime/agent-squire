use serde::Serialize;

use super::model::PatchApplyResult;
use crate::runtime::output::{self, Envelope};

#[derive(Debug, Serialize)]
struct PatchData<'a> {
    results: &'a [PatchApplyResult],
    count: usize,
    failed_count: usize,
}

pub fn print_json(results: &[PatchApplyResult], dry_run: bool) -> anyhow::Result<()> {
    let failed_count = results.iter().filter(|r| !r.success).count();
    let payload = Envelope::new(
        "patch-edit",
        PatchData {
            results,
            count: results.len(),
            failed_count,
        },
    )
    .with_ok(failed_count == 0)
    .with_meta(serde_json::json!({ "dryRun": dry_run }));
    output::print_json(&payload)
}

pub fn print_compact(results: &[PatchApplyResult], dry_run: bool) {
    let action = if dry_run { "Would apply" } else { "Applied" };
    println!("Patch results ({} patch(es))", results.len());

    for result in results {
        if let Some(patch) = &result.patch {
            let status = if result.success { "[OK]" } else { "[X]" };
            let header = if let Some((start, end)) = patch.line_range {
                format!(
                    "# {}:{}",
                    patch.display_path,
                    format_range(Some((start, end)))
                )
            } else {
                format!("# {}", patch.display_path)
            };
            let mut note = result.error.clone().unwrap_or_else(|| {
                if result.status == "applied" {
                    action.into()
                } else {
                    result.status.clone()
                }
            });
            if let Some(line) = result.match_line {
                let mode = result.match_mode.as_deref().unwrap_or("-");
                note = format!("{note} ({mode} @L{line})");
            }
            if let (Some(from), Some(to)) = (&result.indent_from, &result.indent_to) {
                note = format!(
                    "{note}, indent {} -> {}",
                    format_indent(from),
                    format_indent(to)
                );
            }
            println!("{status} {:<18} {} -- {}", result.status, header, note);
        } else {
            println!(
                "[X] parse_error -- {}",
                result.error.as_deref().unwrap_or("unknown parse error")
            );
        }
    }

    let failed = results.iter().filter(|r| !r.success).count();
    if failed == 0 {
        println!("[OK] All patches succeeded.");
    } else {
        println!("[X] {failed} patch(es) failed.");
    }
}

fn format_indent(indent: &str) -> String {
    if indent.is_empty() {
        return "0 spaces".into();
    }

    let spaces = indent.chars().filter(|c| *c == ' ').count();
    let tabs = indent.chars().filter(|c| *c == '\t').count();
    match (spaces, tabs) {
        (0, 1) => "1 tab".into(),
        (0, n) => format!("{n} tabs"),
        (1, 0) => "1 space".into(),
        (n, 0) => format!("{n} spaces"),
        _ => format!("{:?}", indent),
    }
}

fn format_range(range: Option<(Option<usize>, Option<usize>)>) -> String {
    match range {
        None => "Full file".into(),
        Some((Some(start), Some(end))) => format!("L{start}-L{end}"),
        Some((Some(start), None)) => format!("L{start}-"),
        Some((None, Some(end))) => format!("-L{end}"),
        Some((None, None)) => "Full file".into(),
    }
}
