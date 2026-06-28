//! `asq rearrange` — declarative line-range chunk move/copy/delete/reorder.
//!
//! Pipeline: read DSL → [`parser`] → [`Spec`] → [`plan`] → preview/diff, and on
//! `--yes` write back via [`textio`]. Default is dry-run: nothing is written
//! without `--yes`.

mod model;
mod output;
mod parser;
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
    long_about = "Move, copy, delete, or reorder 1-based line-range chunks within one file.\n\nDefine chunks and one action in a small DSL; the spec argument supports literal text, @stdin, @file:path, and @env:NAME. Default mode is dry-run; pass --yes to write."
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

    #[arg(short = 'y', long, help = "Apply changes (write the file)")]
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

    let write = args.yes;
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
