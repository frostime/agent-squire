use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
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
    #[command(
        name = "file-tree",
        alias = "view-tree",
        about = "Show a project directory tree for orientation"
    )]
    Tree(builtins::tree::TreeArgs),

    #[command(
        name = "file-info",
        alias = "fileinfo",
        about = "Inspect file metadata and text/binary format"
    )]
    Info(builtins::info::InfoArgs),

    #[command(
        name = "md-toc",
        alias = "mdtoc",
        about = "Show Markdown headings with 1-based line numbers"
    )]
    Toc(builtins::toc::TocArgs),

    #[command(
        name = "data-toc",
        alias = "datatoc",
        about = "Show JSON/JSONL structure before reading data"
    )]
    DataToc(builtins::data_toc::DataTocArgs),

    #[command(
        name = "md-links",
        alias = "mdlinks",
        about = "Extract Markdown link references"
    )]
    MdLinks(builtins::md_links::MdLinksArgs),

    #[command(
        name = "md-backlinks",
        alias = "mdbacklinks",
        about = "Find Markdown backlinks to files"
    )]
    MdBacklinks(builtins::md_backlinks::MdBacklinksArgs),

    #[command(
        name = "read-range",
        alias = "range",
        about = "Read known 1-based line ranges from one text file"
    )]
    ReadRange(builtins::read_range::ReadRangeArgs),

    #[command(
        name = "patch-edit",
        alias = "patch",
        about = "Apply SEARCH/REPLACE patch blocks"
    )]
    PatchEdit(builtins::patch_edit::PatchEditArgs),

    #[command(
        name = "rearrange",
        alias = "rearr",
        about = "Move, copy, delete, or reorder line-range chunks in one file"
    )]
    Rearrange(builtins::rearrange::RearrangeArgs),

    #[command(
        name = "compose",
        about = "Render agent context templates into bounded UTF-8 output"
    )]
    Compose(builtins::compose::ComposeArgs),

    #[command(
        name = "gather",
        about = "Assemble files, trees, globs, and command output into one prompt"
    )]
    Gather(builtins::gather::GatherArgs),

    #[command(
        name = "img",
        about = "Save clipboard images or start the image web UI"
    )]
    Img(builtins::img::ImgArgs),

    #[command(
        name = "imgweb",
        hide = true,
        about = "Start a local web UI for composing multi-image prompts"
    )]
    ImgWeb(builtins::imgweb::ImgWebArgs),

    #[command(
        name = "tmp",
        alias = "temp",
        about = "Create a temporary file or directory"
    )]
    Tmp(builtins::tmp::TmpArgs),

    #[command(name = "now", about = "Print the current local date and time")]
    Now(NowArgs),

    #[command(about = "List built-in and mapped commands")]
    List(ListArgs),

    #[command(external_subcommand)]
    External(Vec<OsString>),
}

#[derive(Args, Debug)]
#[command(
    long_about = "Print the current local date and time.\n\nUse this when an agent needs a deterministic CLI-accessible timestamp from the local machine. Compact output is human-readable; JSON output returns separate date/time fields plus timezone.",
    after_help = "Examples:\n  squire now\n  squire --print json now"
)]
pub struct NowArgs {}

#[derive(Args, Debug)]
#[command(
    long_about = "List built-in Squire commands and configured external command mappings.\n\nUse this when an agent needs to discover available tools, aliases, and short command summaries in the current environment. JSON output includes built-in command metadata and mapped command configuration.",
    after_help = "Examples:\n  squire list\n  squire --print json list"
)]
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
        CliCommand::DataToc(args) => builtins::data_toc::run(args, &ctx),
        CliCommand::MdLinks(args) => builtins::md_links::run(args, &ctx),
        CliCommand::MdBacklinks(args) => builtins::md_backlinks::run(args, &ctx),
        CliCommand::ReadRange(args) => builtins::read_range::run(args, &ctx),
        CliCommand::PatchEdit(args) => builtins::patch_edit::run(args, &ctx),
        CliCommand::Rearrange(args) => builtins::rearrange::run(args, &ctx),
        CliCommand::Compose(args) => builtins::compose::run(args, &ctx),
        CliCommand::Gather(args) => builtins::gather::run(args, &ctx),
        CliCommand::Img(args) => builtins::img::run(args, &ctx),
        CliCommand::ImgWeb(args) => builtins::imgweb::run(args, &ctx),
        CliCommand::Tmp(args) => builtins::tmp::run(args, &ctx),
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
    let app = Cli::command();

    // Collect builtin subcommands from clap metadata
    let builtins: Vec<_> = app
        .get_subcommands()
        .filter(|cmd| cmd.get_name() != "help" && !cmd.is_hide_set())
        .map(|cmd| {
            let name = cmd.get_name().to_string();
            let aliases: Vec<String> = cmd.get_all_aliases().map(|a| a.to_string()).collect();
            let summary = cmd.get_about().map(|s| s.to_string()).unwrap_or_default();
            (name, aliases, summary)
        })
        .collect();

    if ctx.print == PrintMode::Json {
        let builtin_json: Vec<_> = builtins
            .iter()
            .map(|(name, aliases, summary)| {
                serde_json::json!({ "name": name, "aliases": aliases, "summary": summary })
            })
            .collect();
        let payload = serde_json::json!({
            "ok": true,
            "command": "list",
            "data": { "builtins": builtin_json, "mapped": config.commands },
            "warnings": [],
            "meta": {}
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(0);
    }

    let max_width = builtins.iter().map(|(n, _, _)| n.len()).max().unwrap_or(0);
    println!("Built-in commands:");
    for (name, aliases, summary) in &builtins {
        let alias_str = if aliases.is_empty() {
            String::new()
        } else {
            format!(" (alias: {})", aliases.join(", "))
        };
        println!("  {name:<max_width$}  {summary}{alias_str}");
    }

    if !config.commands.is_empty() {
        println!("\nMapped commands:");
        for (name, cmd) in &config.commands {
            if let Some(summary) = &cmd.summary {
                println!("  {name:<max_width$}  {summary}");
            } else {
                println!("  {name}");
            }
        }
    }
    Ok(0)
}
