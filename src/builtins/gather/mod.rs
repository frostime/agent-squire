mod interactive;
mod model;
mod parser;
mod sources;
mod template;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Args;
use serde::Serialize;

use crate::builtins::compose::compile::compile_template;
use crate::builtins::compose::model::{
    ComposeError, ComposeStatus, OutputInfo, RenderOptions, ShellMode,
};
use crate::builtins::compose::output::temp_output_path_with_prefix;
use crate::builtins::compose::parser::parse_template;
use crate::builtins::compose::render::render_program;
use crate::cli::CommandContext;
use crate::runtime::output::{self as runtime_output, Envelope, PrintMode};

use model::Source;
use parser::{parse_file_content, parse_source};
use template::generate_template;

const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
const DEFAULT_MAX_FILE_BYTES: usize = 1_048_576;
const DEFAULT_MAX_COMMAND_BYTES: usize = 1_048_576;
const DEFAULT_MAX_SPILL_BYTES: usize = 134_217_728;

const GATHER_PROMPT: &str = r#"# Squire gather guide

`asq gather` assembles files, directory/glob file groups, directory trees, and command output into one fenced prompt body.

## Command

```bash
asq gather file:src/main.rs cmd:"git status" 
asq gather --stdout file:src/main.rs:1-80
asq gather -i
```

Default output is a persistent file under the system temp `agent-temp` directory:

```text
output: C:\Users\...\Temp\agent-temp\asq-gather-<timestamp>-<uuid>.md
```

## Sources

- `file:path`
- `file:path:start-end`
- `dir:path` recursively expands files into one DIR group
- `tree:path` renders directory structure
- `glob:pattern` expands files into one GLOB group
- `cmd:command` captures command stdout

## Interactive mode

- `file:`, `dir:`, `tree:`, `glob:` open fzf selectors.
- `prefix:body` adds an explicit source.
- `cmd:` prompts for one command body line.
- Ctrl+D finishes and renders.

## Output format

```text
====== FILE-START: src/main.rs ======
<content>
====== FILE-END ======
```
"#;

#[derive(Args, Debug)]
#[command(
    long_about = "Gather files, directory/glob file groups, trees, and command output into one prompt body. By default, rendered content is written to a persistent file under the system temp agent-temp directory; use --stdout for pipeline mode.",
    after_help = "Examples:\n  asq gather file:src/main.rs cmd:\"git status\"\n  asq gather --stdout file:src/main.rs:1-80\n  asq gather -i"
)]
pub struct GatherArgs {
    #[arg(
        value_name = "SOURCE",
        help = "Sources: file:path, dir:path, tree:path, glob:pattern, cmd:command"
    )]
    pub sources: Vec<String>,

    #[arg(
        short = 'f',
        long = "file",
        value_name = "PATH",
        help_heading = "Sources"
    )]
    pub files: Vec<String>,

    #[arg(
        short = 'd',
        long = "dir",
        value_name = "PATH",
        help_heading = "Sources"
    )]
    pub dirs: Vec<PathBuf>,

    #[arg(
        short = 't',
        long = "tree",
        value_name = "PATH",
        help_heading = "Sources"
    )]
    pub trees: Vec<PathBuf>,

    #[arg(
        short = 'g',
        long = "glob",
        value_name = "PATTERN",
        help_heading = "Sources"
    )]
    pub globs: Vec<String>,

    #[arg(
        short = 'c',
        long = "cmd",
        value_name = "COMMAND",
        help_heading = "Sources"
    )]
    pub commands: Vec<String>,

    #[arg(
        short = 'i',
        long = "interactive",
        help_heading = "Interactive",
        help = "Enter interactive mode; selector lines open fzf"
    )]
    pub interactive: bool,

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
        help_heading = "Output"
    )]
    pub output: Option<PathBuf>,

    #[arg(
        long,
        help_heading = "Output",
        help = "Allow replacing an existing --output file"
    )]
    pub overwrite: bool,

    #[arg(long, value_enum, default_value_t = ShellMode::Auto, help_heading = "Execution")]
    pub shell: ShellMode,

    #[arg(long, default_value_t = DEFAULT_TIMEOUT_SECONDS, value_name = "SECONDS", help_heading = "Execution")]
    pub timeout: u64,

    #[arg(
        long = "total-timeout",
        value_name = "SECONDS",
        help_heading = "Execution"
    )]
    pub total_timeout: Option<u64>,

    #[arg(long = "max-file-bytes", default_value_t = DEFAULT_MAX_FILE_BYTES, value_name = "N", help_heading = "Limits")]
    pub max_file_bytes: usize,

    #[arg(long = "max-command-bytes", default_value_t = DEFAULT_MAX_COMMAND_BYTES, value_name = "N", help_heading = "Limits")]
    pub max_command_bytes: usize,

    #[arg(long = "max-spill-bytes", default_value_t = DEFAULT_MAX_SPILL_BYTES, value_name = "N", help_heading = "Limits")]
    pub max_spill_bytes: usize,

    #[arg(
        long,
        help_heading = "Inspection",
        help = "Print the agent-facing gather guide"
    )]
    pub prompt: bool,
}

#[derive(Debug, Clone)]
enum OutputTarget {
    Temp,
    Stdout,
    File(PathBuf),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GatherMeta {
    schema_version: u8,
    cwd: String,
}

impl GatherMeta {
    fn new(cwd: &Path) -> Self {
        Self {
            schema_version: 1,
            cwd: cwd.display().to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope<'a> {
    ok: bool,
    command: &'static str,
    error: &'a ComposeError,
    warnings: Vec<String>,
    meta: GatherMeta,
}

pub fn run(args: GatherArgs, ctx: &CommandContext) -> Result<u8> {
    if args.prompt {
        println!("{GATHER_PROMPT}");
        return Ok(0);
    }

    let target = resolve_target(args.stdout, args.output.clone())?;
    let sources = collect_sources(&args, &ctx.cwd)?;
    if sources.is_empty() {
        bail!("No sources specified");
    }

    let (template_text, requires_exec) = generate_template(&sources, &ctx.cwd)?;
    let template =
        match parse_template(&template_text).and_then(|template| compile_template(&template)) {
            Ok(program) => program,
            Err(error) => {
                print_error(&error, ctx.print, &ctx.cwd)?;
                return Ok(3);
            }
        };

    let options = RenderOptions {
        cwd: ctx.cwd.clone(),
        allow_exec: requires_exec,
        shell: args.shell,
        timeout_seconds: args.timeout,
        total_timeout_seconds: args.total_timeout,
        max_lines: None,
        max_bytes: None,
        max_file_bytes: args.max_file_bytes,
        max_command_bytes: args.max_command_bytes,
        max_spill_bytes: args.max_spill_bytes,
        fail_on_truncated: false,
    };

    match render_program(&template, &options) {
        Ok(rendered) => {
            let output_info = write_rendered(&target, &rendered.text, args.overwrite)?;
            let status = ComposeStatus {
                output: output_info,
                bytes: rendered.text.len(),
                sources: template.sources.len(),
                truncated: rendered.truncated,
                artifacts: rendered.artifacts,
            };
            print_success(&status, ctx.print, &ctx.cwd)?;
            Ok(0)
        }
        Err(error) => {
            print_error(&error, ctx.print, &ctx.cwd)?;
            Ok(4)
        }
    }
}

fn collect_sources(args: &GatherArgs, cwd: &Path) -> Result<Vec<Source>> {
    let mut sources = Vec::new();

    if args.interactive {
        sources.extend(interactive::read_sources(cwd)?);
    }

    for source in &args.sources {
        sources.push(parse_source(source, cwd)?);
    }
    for file in &args.files {
        let (path, range) = parse_file_content(file)?;
        sources.push(Source::File { path, range });
    }
    sources.extend(args.dirs.iter().cloned().map(|path| Source::Dir { path }));
    sources.extend(args.trees.iter().cloned().map(|path| Source::Tree { path }));
    sources.extend(
        args.globs
            .iter()
            .cloned()
            .map(|pattern| Source::Glob { pattern }),
    );
    sources.extend(
        args.commands
            .iter()
            .cloned()
            .map(|command| Source::Command { command }),
    );

    Ok(sources)
}

fn resolve_target(stdout: bool, output: Option<PathBuf>) -> Result<OutputTarget> {
    match (stdout, output) {
        (true, Some(_)) => bail!("--stdout and --output are mutually exclusive"),
        (true, None) => Ok(OutputTarget::Stdout),
        (false, Some(path)) => Ok(OutputTarget::File(path)),
        (false, None) => Ok(OutputTarget::Temp),
    }
}

fn write_rendered(
    target: &OutputTarget,
    rendered: &str,
    overwrite: bool,
) -> Result<Option<OutputInfo>> {
    match target {
        OutputTarget::Stdout => {
            print!("{rendered}");
            io::stdout().flush()?;
            Ok(None)
        }
        OutputTarget::Temp => {
            let path = temp_output_path_with_prefix("asq-gather")?;
            atomic_write_utf8(&path, rendered, true)?;
            Ok(Some(OutputInfo {
                kind: "temp".into(),
                path: path.display().to_string(),
            }))
        }
        OutputTarget::File(path) => {
            atomic_write_utf8(path, rendered, overwrite)?;
            Ok(Some(OutputInfo {
                kind: "file".into(),
                path: path.display().to_string(),
            }))
        }
    }
}

fn atomic_write_utf8(path: &Path, text: &str, overwrite: bool) -> Result<()> {
    if path.exists() && !overwrite {
        bail!("output file exists: {} (pass --overwrite)", path.display());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temp file in {}", parent.display()))?;
    tmp.write_all(text.as_bytes())?;
    tmp.persist(path)
        .map_err(|err| anyhow::anyhow!("failed to persist {}: {}", path.display(), err.error))?;
    Ok(())
}

fn print_success(status: &ComposeStatus, print: PrintMode, cwd: &Path) -> Result<()> {
    if status.output.is_none() {
        return Ok(());
    }

    match print {
        PrintMode::Json => {
            let payload = Envelope {
                ok: true,
                command: "gather",
                data: status,
                warnings: vec![],
                meta: serde_json::to_value(GatherMeta::new(cwd))?,
            };
            runtime_output::print_json(&payload)?;
        }
        _ => {
            if let Some(output) = &status.output {
                println!("output: {}", output.path);
            }
        }
    }
    Ok(())
}

fn print_error(error: &ComposeError, print: PrintMode, cwd: &Path) -> Result<()> {
    match print {
        PrintMode::Json => {
            let payload = ErrorEnvelope {
                ok: false,
                command: "gather",
                error,
                warnings: vec![],
                meta: GatherMeta::new(cwd),
            };
            eprintln!("{}", serde_json::to_string_pretty(&payload)?);
        }
        _ => eprintln!("error: {}: {}", error.code, error.message),
    }
    Ok(())
}
