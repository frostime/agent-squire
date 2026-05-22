use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use clap::{Args, CommandFactory, Parser, Subcommand};

use crate::builtins;
use crate::external;
use crate::runtime::output::PrintMode;

#[derive(Debug, Clone)]
pub struct CommandContext {
    pub cwd: PathBuf,
    pub print: PrintMode,
}

#[derive(Parser, Debug)]
#[command(
    name = "squire",
    bin_name = "squire",
    version,
    disable_help_subcommand = true,
    about = "A local CLI toolbox for humans and agents.",
    long_about = "Agent Squire packages useful local tools and external command mappings behind a predictable CLI interface for humans and agents."
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "DIR",
        help = "Run as if Squire was started in DIR"
    )]
    pub cwd: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        value_enum,
        default_value_t = PrintMode::Compact,
        value_name = "MODE",
        help = "Output mode: compact, json, ndjson, text, raw"
    )]
    pub print: PrintMode,

    #[arg(long, global = true, help = "Shortcut for --print json")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    #[command(name = "file-tree", alias = "view-tree", about = "Show a project directory tree")]
    Tree(builtins::tree::TreeArgs),

    #[command(
        name = "file-info",
        alias = "fileinfo",
        about = "Inspect file metadata and text/binary format"
    )]
    Info(builtins::info::InfoArgs),

    #[command(name = "md-toc", alias = "mdtoc", about = "Show Markdown headings with line numbers")]
    Toc(builtins::toc::TocArgs),

    #[command(
        name = "patch-edit",
        alias = "patch",
        about = "Apply SEARCH/REPLACE patch blocks"
    )]
    PatchEdit(builtins::patch_edit::PatchEditArgs),

    #[command(name = "now", about = "Print the current date and time")]
    Now(NowArgs),

    #[command(about = "List built-in and mapped commands")]
    List(ListArgs),

    #[command(external_subcommand)]
    External(Vec<OsString>),
}

#[derive(Args, Debug)]
pub struct NowArgs {}

#[derive(Args, Debug)]
pub struct ListArgs {}

pub fn main_entry() -> ExitCode {
    match try_main() {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn try_main() -> Result<u8> {
    let cli = Cli::parse();
    let print = if cli.json { PrintMode::Json } else { cli.print };

    if let Some(cwd) = &cli.cwd {
        std::env::set_current_dir(cwd)
            .with_context(|| format!("failed to set --cwd {}", cwd.display()))?;
    }

    let cwd = std::env::current_dir().context("failed to read current directory")?;
    let ctx = CommandContext { cwd, print };

    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        println!();
        return Ok(0);
    };

    match command {
        CliCommand::Tree(args) => builtins::tree::run(args, &ctx),
        CliCommand::Info(args) => builtins::info::run(args, &ctx),
        CliCommand::Toc(args) => builtins::toc::run(args, &ctx),
        CliCommand::PatchEdit(args) => builtins::patch_edit::run(args, &ctx),
        CliCommand::Now(_) => builtins::now::run(&ctx),
        CliCommand::List(_) => run_list(&ctx),
        CliCommand::External(raw) => {
            if raw.is_empty() {
                bail!("missing external command name");
            }
            let mut iter = raw.into_iter();
            let name = iter
                .next()
                .expect("checked non-empty")
                .into_string()
                .map_err(|_| anyhow::anyhow!("external command name must be valid UTF-8"))?;
            let args = iter.collect::<Vec<_>>();
            external::run_mapped(&name, args, &ctx)
        }
    }
}

fn run_list(ctx: &CommandContext) -> Result<u8> {
    let config = external::load_config();
    if ctx.print == PrintMode::Json {
        let payload = serde_json::json!({
            "ok": true,
            "command": "list",
            "data": {
                "builtins": [
                    {"name": "file-tree", "aliases": ["view-tree"], "summary": "Show a project directory tree"},
                    {"name": "file-info", "aliases": ["fileinfo"], "summary": "Inspect file metadata and text/binary format"},
                    {"name": "md-toc", "aliases": ["mdtoc"], "summary": "Show Markdown headings with line numbers"},
                    {"name": "patch-edit", "aliases": ["patch"], "summary": "Apply SEARCH/REPLACE patch blocks"},
                    {"name": "now", "aliases": [], "summary": "Print the current date and time"},
                    {"name": "list", "aliases": [], "summary": "List built-in and mapped commands"}
                ],
                "mapped": config.commands
            },
            "warnings": [],
            "meta": {}
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(0);
    }

    println!("Built-in commands:");
    println!("  file-tree     Show a project directory tree. Alias: view-tree");
    println!("  file-info     Inspect file metadata and text/binary format. Alias: fileinfo");
    println!("  md-toc        Show Markdown heading outline. Alias: mdtoc");
    println!("  patch-edit    Apply SEARCH/REPLACE patch blocks. Alias: patch");
    println!("  now           Print the current date and time");
    println!("  list          List built-in and mapped commands");

    if !config.commands.is_empty() {
        println!("\nMapped commands:");
        for (name, cmd) in config.commands {
            if let Some(summary) = cmd.summary {
                println!("  {name:<13} {summary}");
            } else {
                println!("  {name}");
            }
        }
    }
    Ok(0)
}
