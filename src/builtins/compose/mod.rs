pub(crate) mod compile;
pub(crate) mod help_doc;
pub(crate) mod model;
mod modifiers;
pub(crate) mod output;
pub(crate) mod parser;
pub(crate) mod render;
mod sources;
mod text;

use anyhow::{Result, bail};
use clap::Args;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

use crate::cli::CommandContext;
use crate::runtime::output::{self as runtime_output, Envelope, PrintMode};

use compile::compile_template;
use help_doc::COMPOSE_PROMPT;
use model::{ComposeError, ComposeStatus, FailureCase, RenderOptions, ShellMode, SourceInfo};
use output::{
    ComposeMeta, print_check_ok, print_error, print_success, resolve_target, write_rendered,
};
use parser::parse_template;
use render::render_program;
use text::decode_text;

const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
const DEFAULT_MAX_FILE_BYTES: usize = 1_048_576;
const DEFAULT_MAX_COMMAND_BYTES: usize = 1_048_576;
const DEFAULT_MAX_SPILL_BYTES: usize = 134_217_728;

const LONG_ABOUT: &str = "Render deterministic agent context templates. By default, rendered content is written to a persistent file under the system temp agent-temp directory; use --stdout for pipeline mode. Use --prompt for the agent-facing template guide.";

const AFTER_HELP: &str = "Workflow:
    asq compose -t context.tpl.md --check
    asq compose -t context.tpl.md --list-sources
    asq compose -t context.tpl.md

Examples:
    asq compose -t context.tpl.md
    asq compose --template '${{file: README.md |> head: 80}}'
    asq compose -t context.tpl.md --stdout
    asq compose -t context.tpl.md --allow-exec
";

#[derive(Args, Debug)]
#[command(
    long_about = LONG_ABOUT,
    after_help = AFTER_HELP
)]
pub struct ComposeArgs {
    #[arg(
        short = 't',
        long = "template-file",
        value_name = "PATH",
        help_heading = "Template Input",
        help = "Read template text from PATH"
    )]
    pub template_file: Option<PathBuf>,

    #[arg(
        long,
        value_name = "TEXT",
        help_heading = "Template Input",
        help = "Inline template text; prefer --template-file for complex templates"
    )]
    pub template: Option<String>,

    #[arg(
        long,
        help_heading = "Output",
        help = "Write rendered body to stdout instead of a file"
    )]
    pub stdout: bool,

    #[arg(
        short = 'o',
        long = "output",
        value_name = "PATH",
        help_heading = "Output",
        help = "Write rendered body to PATH"
    )]
    pub output: Option<PathBuf>,

    #[arg(
        long,
        help_heading = "Output",
        help = "Allow replacing an existing --output file"
    )]
    pub overwrite: bool,

    #[arg(long, help_heading = "Execution", help = "Enable exec: sources")]
    pub allow_exec: bool,

    #[arg(
        long,
        value_enum,
        default_value_t = ShellMode::Auto,
        help_heading = "Execution",
        help = "Shell used for exec: sources"
    )]
    pub shell: ShellMode,

    #[arg(
        long,
        default_value_t = DEFAULT_TIMEOUT_SECONDS,
        value_name = "SECONDS",
        help_heading = "Execution",
        help = "Default timeout for one exec: source"
    )]
    pub timeout: u64,

    #[arg(
        long = "total-timeout",
        value_name = "SECONDS",
        help_heading = "Execution",
        help = "Total render-phase wall-clock budget"
    )]
    pub total_timeout: Option<u64>,

    #[arg(
        long = "max-lines",
        value_name = "N",
        help_heading = "Limits",
        help = "Maximum rendered output lines after interpolation"
    )]
    pub max_lines: Option<usize>,

    #[arg(
        long = "max-bytes",
        value_name = "N",
        help_heading = "Limits",
        help = "Maximum rendered output bytes after interpolation"
    )]
    pub max_bytes: Option<usize>,

    #[arg(
        long = "max-file-bytes",
        default_value_t = DEFAULT_MAX_FILE_BYTES,
        value_name = "N",
        help_heading = "Limits",
        help = "Maximum bytes read from one file: source"
    )]
    pub max_file_bytes: usize,

    #[arg(
        long = "max-command-bytes",
        default_value_t = DEFAULT_MAX_COMMAND_BYTES,
        value_name = "N",
        help_heading = "Limits",
        help = "Maximum exec stdout/stderr bytes kept in rendered text"
    )]
    pub max_command_bytes: usize,

    #[arg(
        long = "max-spill-bytes",
        default_value_t = DEFAULT_MAX_SPILL_BYTES,
        value_name = "N",
        help_heading = "Limits",
        help = "Per-run byte budget for exec spill artifact files"
    )]
    pub max_spill_bytes: usize,

    #[arg(
        long,
        help_heading = "Limits",
        help = "Fail instead of inserting truncation markers"
    )]
    pub fail_on_truncated: bool,

    #[arg(
        long,
        help_heading = "Validation",
        help = "Parse and validate without resolving sources"
    )]
    pub check: bool,

    #[arg(
        long,
        help_heading = "Validation",
        help = "List discovered sources without reading or executing them"
    )]
    pub list_sources: bool,

    #[arg(
        long,
        help_heading = "Validation",
        help = "Print the agent-facing compose template guide"
    )]
    pub prompt: bool,
}

#[derive(Debug, Serialize)]
struct SourceListData {
    sources: Vec<SourceInfo>,
}

pub fn run(args: ComposeArgs, ctx: &CommandContext) -> Result<u8> {
    if args.prompt {
        println!("{COMPOSE_PROMPT}");
        return Ok(0);
    }

    if args.template_file.is_some() == args.template.is_some() {
        bail!("use exactly one template source: --template-file or --template");
    }

    let target = resolve_target(args.stdout, args.output.clone())?;
    let options = RenderOptions {
        cwd: ctx.cwd.clone(),
        allow_exec: args.allow_exec,
        shell: args.shell,
        timeout_seconds: args.timeout,
        total_timeout_seconds: args.total_timeout,
        max_lines: args.max_lines,
        max_bytes: args.max_bytes,
        max_file_bytes: args.max_file_bytes,
        max_command_bytes: args.max_command_bytes,
        max_spill_bytes: args.max_spill_bytes,
        fail_on_truncated: args.fail_on_truncated,
    };

    // Phase boundary: parse and compile must stay side-effect free so --check and
    // --list-sources can validate untrusted templates without touching sources.
    let template_text = match load_template(&args) {
        Ok(template_text) => template_text,
        Err(error) => {
            print_error(&error, ctx.print, &ctx.cwd)?;
            return Ok(exit_code_for(&error));
        }
    };
    let template = match parse_template(&template_text) {
        Ok(template) => template,
        Err(error) => {
            print_error(&error, ctx.print, &ctx.cwd)?;
            return Ok(exit_code_for(&error));
        }
    };
    let program = match compile_template(&template) {
        Ok(program) => program,
        Err(error) => {
            print_error(&error, ctx.print, &ctx.cwd)?;
            return Ok(exit_code_for(&error));
        }
    };

    if args.check {
        print_check_ok(ctx.print, program.sources.len(), &ctx.cwd)?;
        return Ok(0);
    }

    if args.list_sources {
        print_sources(&program.sources, ctx.print, &ctx.cwd)?;
        return Ok(0);
    }

    match render_program(&program, &options) {
        Ok(rendered) => {
            let output_info = write_rendered(&target, &rendered.text, args.overwrite)?;
            let status = ComposeStatus {
                output: output_info,
                bytes: rendered.text.len(),
                sources: program.sources.len(),
                truncated: rendered.truncated,
                artifacts: rendered.artifacts,
            };
            print_success(&status, ctx.print, &ctx.cwd)?;
            Ok(0)
        }
        Err(error) => {
            print_error(&error, ctx.print, &ctx.cwd)?;
            Ok(exit_code_for(&error))
        }
    }
}

fn load_template(args: &ComposeArgs) -> model::ComposeResult<String> {
    if let Some(path) = &args.template_file {
        let bytes = fs::read(path).map_err(|err| {
            let case = if err.kind() == std::io::ErrorKind::NotFound {
                FailureCase::NotFound
            } else {
                FailureCase::Error
            };
            ComposeError::new(
                "template_read_failed",
                Some(case),
                format!("Failed to read template {}: {err}", path.display()),
            )
        })?;
        return decode_text(&bytes, &path.display().to_string());
    }
    Ok(args.template.clone().expect("validated template source"))
}

fn print_sources(sources: &[SourceInfo], print: PrintMode, cwd: &std::path::Path) -> Result<()> {
    match print {
        PrintMode::Json => {
            let payload = Envelope::new(
                "compose",
                SourceListData {
                    sources: sources.to_vec(),
                },
            )
            .with_meta(serde_json::to_value(ComposeMeta::new(cwd))?);
            runtime_output::print_json(&payload)?;
        }
        _ => {
            for source in sources {
                let argument = if source.argument.is_empty() {
                    String::new()
                } else {
                    format!(": {}", source.argument)
                };
                println!(
                    "{}  {}{}  {}:{}",
                    source.index,
                    source.kind,
                    argument,
                    source.location.line,
                    source.location.column
                );
            }
        }
    }
    Ok(())
}

fn exit_code_for(error: &ComposeError) -> u8 {
    match error.case {
        Some(FailureCase::Parse) => 3,
        Some(FailureCase::NotFound) => 4,
        Some(FailureCase::Timeout) => 6,
        Some(FailureCase::Limit) => 9,
        Some(FailureCase::Error) if error.code == "exec_disabled" => 5,
        Some(FailureCase::Error) if error.code == "exec_nonzero" => 7,
        Some(
            FailureCase::Error
            | FailureCase::Encoding
            | FailureCase::Binary
            | FailureCase::Modifier
            | FailureCase::Range,
        ) => 4,
        None => 1,
    }
}
