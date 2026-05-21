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
    about = "A local CLI toolbox for humans and agents.",
    long_about = "Agent Squire packages useful local tools and external command mappings behind a predictable CLI interface for humans and agents."
)]
pub struct Cli {
    #[arg(long, global = true, value_name = "DIR", help = "Run as if Squire was started in DIR")]
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
    #[command(alias = "view-tree", about = "Show a project directory tree")]
    Tree(builtins::tree::TreeArgs),

    #[command(alias = "fileinfo", about = "Inspect file metadata and text/binary format")]
    Info(builtins::info::InfoArgs),

    #[command(alias = "mdtoc", about = "Show Markdown headings with line numbers")]
    Toc(builtins::toc::TocArgs),

    #[command(name = "patch-edit", alias = "patch", about = "Apply SEARCH/REPLACE patch blocks")]
    PatchEdit(builtins::patch_edit::PatchEditArgs),

    #[command(about = "List built-in and mapped commands")]
    List(ListArgs),

    #[command(about = "Inspect external command mappings")]
    Map(MapArgs),

    #[command(external_subcommand)]
    External(Vec<OsString>),
}

#[derive(Args, Debug)]
pub struct ListArgs {}

#[derive(Args, Debug)]
pub struct MapArgs {
    #[command(subcommand)]
    pub command: Option<MapCommand>,
}

#[derive(Subcommand, Debug)]
pub enum MapCommand {
    #[command(about = "List mapped commands")]
    List,

    #[command(about = "Show mapping config locations and format")]
    Help,
}

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
        CliCommand::List(_) => run_list(&ctx),
        CliCommand::Map(args) => run_map(args, &ctx),
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
                    {"name": "tree", "aliases": ["view-tree"], "summary": "Show a project directory tree"},
                    {"name": "info", "aliases": ["fileinfo"], "summary": "Inspect file metadata and text/binary format"},
                    {"name": "toc", "aliases": ["mdtoc"], "summary": "Show Markdown headings with line numbers"},
                    {"name": "patch-edit", "aliases": ["patch"], "summary": "Apply SEARCH/REPLACE patch blocks"},
                    {"name": "list", "aliases": [], "summary": "List built-in and mapped commands"},
                    {"name": "map", "aliases": [], "summary": "Inspect external command mappings"}
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
    println!("  tree          Show a project directory tree. Alias: view-tree");
    println!("  info          Inspect file metadata and text/binary format. Alias: fileinfo");
    println!("  toc           Show Markdown heading outline. Alias: mdtoc");
    println!("  patch-edit    Apply SEARCH/REPLACE patch blocks. Alias: patch");
    println!("  list          List built-in and mapped commands");
    println!("  map           Inspect external command mappings");

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

fn run_map(args: MapArgs, _ctx: &CommandContext) -> Result<u8> {
    match args.command.unwrap_or(MapCommand::List) {
        MapCommand::List => {
            let config = external::load_config();
            if config.commands.is_empty() {
                println!("No mapped commands found.");
                println!("Checked:");
                for path in external::config_paths() {
                    println!("  {}", path.display());
                }
                return Ok(0);
            }
            for (name, cmd) in config.commands {
                println!("{name}");
                if let Some(summary) = cmd.summary {
                    println!("  summary: {summary}");
                }
                println!("  run: {}", cmd.run.join(" "));
                println!("  print_aware: {}", cmd.print_aware);
                println!("  expand_args: {}", cmd.expand_args);
            }
            Ok(0)
        }
        MapCommand::Help => {
            println!("{}", external::MAP_HELP);
            Ok(0)
        }
    }
}
