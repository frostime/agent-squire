//! `asq rearrange` — Arrange state-transition DSL for line-range material reuse.
//!
//! Pipeline: read DSL → parse AST → validate pre-state snapshot/provenance →
//! preview, and on `--yes` write/delete/create target files.

mod ast;
mod error;
mod output;
mod parser;
mod path;
mod plan;
mod textio;

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use crate::cli::CommandContext;
use crate::runtime::input;

const REARRANGE_PROMPT: &str = include_str!("prompt.md");

#[derive(Args, Debug)]
#[command(
    long_about = "Rewrite files with the Arrange state-transition DSL. The spec argument supports literal text, @stdin, @file:path, and @env:NAME. Default mode is dry-run; pass --yes to write."
)]
pub struct RearrangeArgs {
    #[arg(help = "Spec content or input source: literal, @stdin, @file:path, @env:NAME")]
    pub spec: Option<String>,

    #[arg(
        short = 'f',
        long = "file",
        value_name = "PATH",
        help = "Read spec from a file"
    )]
    pub file: Option<PathBuf>,

    #[arg(long, help = "Read spec from stdin")]
    pub stdin: bool,

    #[arg(long, help = "Validate and preview without writing (default)")]
    pub dry_run: bool,

    #[arg(short = 'y', long, help = "Apply changes")]
    pub yes: bool,

    #[arg(long, help = "Print the rearrange DSL and CLI guide")]
    pub prompt: bool,
}

pub fn run(args: RearrangeArgs, ctx: &CommandContext) -> Result<u8> {
    if args.prompt {
        println!("{REARRANGE_PROMPT}");
        return Ok(0);
    }

    let spec_text = read_spec_source(&args)?;
    if spec_text.trim().is_empty() {
        println!("No input. Skipped.");
        return Ok(0);
    }

    let write = args.yes && !args.dry_run;
    match plan::execute(&spec_text, &ctx.cwd, write) {
        Ok(outcome) => {
            output::render(&outcome, write, ctx.print);
            Ok(0)
        }
        Err(err) => {
            output::render_error(&err, ctx.print);
            Ok(1)
        }
    }
}

fn read_spec_source(args: &RearrangeArgs) -> Result<String> {
    let source = if args.stdin {
        "@stdin".to_string()
    } else if let Some(path) = &args.file {
        format!("@file:{}", path.display())
    } else {
        args.spec.clone().unwrap_or_else(|| "@stdin".into())
    };
    input::read_text_source(&source)
}
