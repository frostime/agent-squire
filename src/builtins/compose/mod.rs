mod compile;
mod model;
mod modifiers;
mod output;
mod parser;
mod render;
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

const COMPOSE_PROMPT: &str = r#"# Squire compose template guide

`asq compose` renders agent context templates into UTF-8 output files.

## Command

```bash
asq compose -t context.tpl.md
asq compose --template "${{file: README.md |> head: 80}}"
asq compose -t context.tpl.md --stdout
asq compose -t context.tpl.md --allow-exec
asq compose -t context.tpl.md --allow-exec --max-command-bytes 1048576 --max-spill-bytes 134217728
```

Default output is a persistent file under the system temp directory:

```text
%TEMP%/agent-temp/asq-compose-<timestamp>-<uuid>.md
```

Use `--stdout` only when the rendered body should be piped to another command.

## Syntax

```md
${{source |> stage |> stage}}
```

Source commands appear first: `stdin`, `file: path`, `env: NAME`, `exec: command`.
No-argument commands may omit the colon: `stdin`, `trim`, `stdout`, `stderr`.
Commands with bodies use `name: body`.

Stage command roles:

- runtime: `timeout: 5`
- stream: `stdout`, `stderr`
- transform: `lines: 1-END`, `head: 80`, `trim`, `indent: 2`
- policy: `fallback: ""`, `on-404: "missing"`, `on-error: "failed"`

Text transforms run left-to-right. Runtime controls, stream selectors, and failure policies are normalized by role.

## Multiline

```md
${{
  file: README.md
  |> lines: 1-END
  |> indent: 2
}}
```

Multiline literal values use JSON string escapes:

```md
${{file: missing.md |> fallback: "line1\nline2"}}
```

## Safety

`exec:` is disabled unless `--allow-exec` is passed. `stdin` fails when stdin is an interactive terminal, preventing accidental hangs.

Large `exec:` stdout/stderr streams are drained while the child runs. Rendered text keeps the first `--max-command-bytes`; excess bytes spill to temp files under a per-render `--max-spill-bytes` budget. `--total-timeout` is the whole render-phase wall-clock budget.
"#;

#[derive(Args, Debug)]
#[command(
    long_about = "Render deterministic agent context templates. By default, rendered content is written to a persistent file under the system temp agent-temp directory; use --stdout for pipeline mode. Use --prompt for the agent-facing template guide.",
    after_help = "Examples:\n  asq compose -t context.tpl.md\n  asq compose --template '${{file: README.md |> head: 80}}'\n  asq compose -t context.tpl.md --stdout\n  asq compose -t context.tpl.md --allow-exec"
)]
pub struct ComposeArgs {
    #[arg(short = 't', long = "template-file", value_name = "PATH")]
    pub template_file: Option<PathBuf>,

    #[arg(long, value_name = "TEXT")]
    pub template: Option<String>,

    #[arg(long, help = "Write rendered body to stdout instead of a file")]
    pub stdout: bool,

    #[arg(short = 'o', long = "output", value_name = "PATH")]
    pub output: Option<PathBuf>,

    #[arg(long, help = "Allow replacing an existing --output file")]
    pub overwrite: bool,

    #[arg(long, help = "Enable exec: sources")]
    pub allow_exec: bool,

    #[arg(long, value_enum, default_value_t = ShellMode::Auto)]
    pub shell: ShellMode,

    #[arg(long, default_value_t = DEFAULT_TIMEOUT_SECONDS, value_name = "SECONDS")]
    pub timeout: u64,

    #[arg(long = "total-timeout", value_name = "SECONDS")]
    pub total_timeout: Option<u64>,

    #[arg(long = "max-lines", value_name = "N")]
    pub max_lines: Option<usize>,

    #[arg(long = "max-bytes", value_name = "N")]
    pub max_bytes: Option<usize>,

    #[arg(long = "max-file-bytes", default_value_t = DEFAULT_MAX_FILE_BYTES, value_name = "N")]
    pub max_file_bytes: usize,

    #[arg(long = "max-command-bytes", default_value_t = DEFAULT_MAX_COMMAND_BYTES, value_name = "N")]
    pub max_command_bytes: usize,

    #[arg(long = "max-spill-bytes", default_value_t = DEFAULT_MAX_SPILL_BYTES, value_name = "N")]
    pub max_spill_bytes: usize,

    #[arg(long, help = "Fail instead of inserting truncation markers")]
    pub fail_on_truncated: bool,

    #[arg(long, help = "Parse and validate without resolving sources")]
    pub check: bool,

    #[arg(
        long,
        help = "List discovered sources without reading or executing them"
    )]
    pub list_sources: bool,

    #[arg(long, help = "Print the agent-facing compose template guide")]
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
            let payload = Envelope {
                ok: true,
                command: "compose",
                data: SourceListData {
                    sources: sources.to_vec(),
                },
                warnings: vec![],
                meta: serde_json::to_value(ComposeMeta::new(cwd))?,
            };
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
