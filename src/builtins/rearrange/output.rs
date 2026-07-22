//! Render `rearrange` outcomes.

use crate::builtins::rearrange::error::RearrangeError;
use crate::builtins::rearrange::plan::Outcome;
use crate::runtime::output::{Envelope, PrintMode, print_json};

pub fn render(outcome: &Outcome, written_mode: bool, mode: PrintMode) {
    match mode {
        PrintMode::Json => render_json(outcome, written_mode),
        _ => render_compact(outcome, written_mode),
    }
}

pub fn render_error(err: &RearrangeError, mode: PrintMode) {
    match mode {
        PrintMode::Json => {
            let payload = Envelope::new("rearrange", serde_json::Value::Null)
                .with_ok(false)
                .with_meta(serde_json::json!({
                    "error_code": err.code.as_str(),
                    "message": err.message,
                    "line": err.line,
                }));
            let _ = print_json(&payload);
        }
        _ => eprintln!("error: {err}"),
    }
}

fn render_compact(outcome: &Outcome, written_mode: bool) {
    let state = if written_mode && outcome.changed {
        "written"
    } else if written_mode {
        "no-op"
    } else {
        "dry-run"
    };
    println!("rearrange {} files ({state})", outcome.targets.len());
    println!();

    for share in &outcome.shares {
        println!("share {} = {}", share.slug, share.path);
        for item in &share.items {
            println!("  {} = {} -> {} lines", item.name, item.range, item.lines);
        }
        println!();
    }

    for target in &outcome.targets {
        if let Some(slug) = &target.slug {
            println!("target {slug} = {}", target.path);
        } else {
            println!("target {}", target.path);
        }
        println!("  before: {}", target.before);
        println!("  after : {}", target.after);
        if !target.exports.is_empty() {
            println!("  exports: {}", target.exports.join(", "));
        }
        for gap in &target.gaps {
            println!("  gap {} = {} -> {} lines", gap.name, gap.range, gap.lines);
        }
        println!("  effects: {}", target.effects.join(", "));
        println!();
    }

    if !outcome.changed {
        println!("No change.");
    } else if written_mode {
        println!(
            "{} target file(s) changed.",
            outcome.targets.iter().filter(|t| t.changed).count()
        );
    } else {
        println!("No file written. Pass --yes to apply.");
    }
}

fn render_json(outcome: &Outcome, written_mode: bool) {
    let payload = Envelope::new(
        "rearrange",
        serde_json::json!({
            "written": written_mode && outcome.changed,
            "changed": outcome.changed,
            "shares": outcome.shares,
            "targets": outcome.targets,
        }),
    );
    let _ = print_json(&payload);
}
