mod io;
mod match_apply;
mod model;
mod output;
mod parse;
mod text;

use std::env;
use std::fs;
use std::io::{self as stdio, BufRead, IsTerminal, Write};
use std::process::Command;

use anyhow::{Context, Result, bail};
use chrono::Local;
use clap::Args;

use crate::builtins::patch_edit::io::read_target_text_with_encoding;
use crate::cli::CommandContext;
use crate::runtime::input;
use crate::runtime::output::PrintMode;

pub use match_apply::{
    apply_parsed_patches, apply_parsed_patches_with_options, apply_patches,
    apply_patches_with_options,
};
pub use model::{PatchApplyOptions, PatchApplyResult, PatchBlock, PatchOperation};
pub use parse::parse_patches;

const PATCH_PROMPT: &str = r#"# Squire patch-edit format

`asq patch` accepts structured patch blocks for local file edit.

## Patch Block | 3 Type

1) Targeted edit by str replace
```patch
# <path>[:<range>]
<<<<<<< SEARCH
old content
=======
new content
>>>>>>> REPLACE
```
Line range (optional): 1based; `L10-L25`, `L10-`, `-L25`; only in SEARCH method.

2) Create new file
```patch
# <path>
<<<<<<< CREATE
=======
new file content
>>>>>>> REPLACE
````
3) Full overwrite
```patch
# <path>
<<<<<<< OVERWRITE
=======
full replacement content
>>>>>>> REPLACE
```

---

- Fense-markers (<,=,>) appear alone per line, with 7 char repetitions
- Section before '===' of CREATE and OVERWRITE MUST be empty

## Multi-block bundles

Multi-blocks patch is allowed to include concise human-readable explanations surrounding patch blocks. i.e. A text report includes
patch blocks.

```example
First, xxx

[[Patch Block]]

Next, xxx

[[Patch Block]]
```

WARN: Multi-blocks targeting the same file are matched against the original content. Overlapping matches cause all related blocks to fail.
"#;

#[derive(Args, Debug)]
#[command(
    long_about = "Apply SEARCH/REPLACE patch blocks. The patch argument supports literal text, @stdin, @file:path, and @env:NAME."
)]
pub struct PatchEditArgs {
    #[arg(help = "Patch content or input source: literal, @stdin, @file:path, @env:NAME")]
    pub patch: Option<String>,

    #[arg(
        short = 'f',
        long = "file",
        value_name = "PATH",
        help = "Read patch text from a file"
    )]
    pub file: Option<std::path::PathBuf>,

    #[arg(long, help = "Read patch text from stdin")]
    pub stdin: bool,

    #[arg(
        short = 'i',
        long = "input",
        help = "Interactively enter multiline patch text, dry-run it, then approve applying it"
    )]
    pub input: bool,

    #[arg(long, help = "Validate without modifying files")]
    pub dry_run: bool,

    #[arg(short = 'y', long, help = "Required for non-dry-run writes")]
    pub yes: bool,

    #[arg(long, help = "Enable smart indent matching for SEARCH blocks")]
    pub smart_indent: bool,

    #[arg(long, help = "Print the patch format specification")]
    pub prompt: bool,
}

pub fn run(args: PatchEditArgs, ctx: &CommandContext) -> Result<u8> {
    if args.prompt {
        println!("{PATCH_PROMPT}");
        return Ok(0);
    }

    let mut source_count = 0;
    if args.patch.is_some() {
        source_count += 1;
    }
    if args.file.is_some() {
        source_count += 1;
    }
    if args.stdin {
        source_count += 1;
    }
    if args.input {
        source_count += 1;
    }
    if source_count > 1 {
        bail!("use exactly one patch source: positional PATCH, --file, --stdin, or --input");
    }

    if args.input {
        return run_interactive(args.dry_run, args.smart_indent, ctx);
    }

    if !args.dry_run && !args.yes {
        bail!("patch-edit requires --yes for writes; use --dry-run to validate without writing");
    }

    let source = if args.stdin {
        "@stdin".to_string()
    } else if let Some(path) = args.file {
        format!("@file:{}", path.display())
    } else {
        args.patch.unwrap_or_else(|| "@stdin".into())
    };

    let patch_text = input::read_text_source(&source)?;
    if patch_text.trim().is_empty() {
        println!("No input. Skipped.");
        return Ok(0);
    }

    run_once(&patch_text, args.dry_run, args.smart_indent, ctx)
}

fn run_once(
    patch_text: &str,
    dry_run: bool,
    smart_indent: bool,
    ctx: &CommandContext,
) -> Result<u8> {
    let results = apply_patches_with_options(
        patch_text,
        &ctx.cwd,
        PatchApplyOptions {
            dry_run,
            smart_indent,
        },
    );
    let all_success = results.iter().all(|r| r.success);

    match ctx.print {
        PrintMode::Json => output::print_json(&results, dry_run)?,
        _ => output::print_compact(&results, dry_run),
    }

    Ok(if all_success { 0 } else { 1 })
}

fn run_interactive(dry_run_only: bool, smart_indent: bool, ctx: &CommandContext) -> Result<u8> {
    let patch_text = read_patch_text_interactive()?;
    if patch_text.trim().is_empty() {
        println!("No input. Skipped.");
        return Ok(0);
    }

    eprintln!("Dry-run preview:");
    let preview_results = apply_patches_with_options(
        &patch_text,
        &ctx.cwd,
        PatchApplyOptions {
            dry_run: true,
            smart_indent,
        },
    );
    let preview_success = preview_results.iter().all(|r| r.success);
    match ctx.print {
        PrintMode::Json => output::print_json(&preview_results, true)?,
        _ => output::print_compact(&preview_results, true),
    }

    if dry_run_only {
        return Ok(if preview_success { 0 } else { 1 });
    }
    if !preview_success {
        eprintln!("Dry-run failed; no changes applied.");
        return Ok(1);
    }

    if confirm("Show unified diff?", false)? {
        print_patch_diff(&preview_results)?;
    }

    if !confirm("Apply these patches?", false)? {
        eprintln!("Aborted.");
        return Ok(1);
    }

    run_once(&patch_text, false, smart_indent, ctx)
}

fn read_patch_text_interactive() -> Result<String> {
    if let Some(text) = read_patch_text_from_editor()? {
        return Ok(text);
    }

    read_patch_text_from_terminal()
}

fn read_patch_text_from_editor() -> Result<Option<String>> {
    let Some(editor) = env::var_os("EDITOR").or_else(|| env::var_os("VISUAL")) else {
        return Ok(None);
    };

    let temp_dir = std::env::temp_dir().join("asq-temp");
    fs::create_dir_all(&temp_dir).with_context(|| {
        format!(
            "failed to create temporary directory {}",
            temp_dir.display()
        )
    })?;
    let timestamp = Local::now().format("%Y%m%dT%H%M%S");
    let path = temp_dir.join(format!("patch-{timestamp}.md"));
    std::fs::write(&path, "")
        .with_context(|| format!("failed to create temporary patch file {}", path.display()))?;

    eprintln!("Opening patch editor: {}", editor.to_string_lossy());
    eprintln!("Patch file: {}", path.display());
    eprintln!("Save and close the editor to continue.");

    let mut command = editor_command(&editor.to_string_lossy())?;
    command.arg(&path);
    let status = command.status().context("failed to launch $EDITOR")?;

    if !status.success() {
        bail!("editor exited with status {status}");
    }

    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read temporary patch file {}", path.display()))?;
    Ok(Some(text))
}

fn editor_command(editor: &str) -> Result<Command> {
    let parts = split_editor_command(editor);
    let Some((program, args)) = parts.split_first() else {
        bail!("$EDITOR is empty");
    };

    let mut command = Command::new(program);
    command.args(args);
    Ok(command)
}

fn split_editor_command(editor: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for ch in editor.chars() {
        match ch {
            '\'' | '"' if quote == Some(ch) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            ch if ch.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

fn read_patch_text_from_terminal() -> Result<String> {
    if !stdio::stdin().is_terminal() {
        bail!("--input requires an interactive terminal or $EDITOR; use --stdin for piped input");
    }

    eprintln!(
        "Interactive patch input. Paste/write patch text, then enter a single '.' line to submit."
    );
    eprintln!("EOF also submits: Ctrl+D on Unix/macOS, Ctrl+Z then Enter on Windows.");

    let stdin = stdio::stdin();
    let mut lines = stdin.lock().lines();
    let mut text = String::new();

    loop {
        eprint!("> ");
        stdio::stderr().flush()?;
        let Some(line) = lines.next() else {
            break;
        };
        let line = line?;
        if line.trim_end() == "." || line.trim_end() == "<<ASQ-PATCH-END>>" {
            break;
        }
        text.push_str(&line);
        text.push('\n');
    }

    Ok(text)
}

fn confirm(prompt: &str, default: bool) -> Result<bool> {
    if !stdio::stdin().is_terminal() {
        return Ok(default);
    }

    let suffix = if default { "[Y/n]" } else { "[y/N]" };
    eprint!("{prompt} {suffix} ");
    stdio::stderr().flush()?;

    let mut answer = String::new();
    stdio::stdin().read_line(&mut answer)?;
    let answer = answer.trim();
    if answer.is_empty() {
        return Ok(default);
    }

    Ok(matches!(answer, "y" | "Y" | "yes" | "YES" | "Yes"))
}

fn print_patch_diff(results: &[PatchApplyResult]) -> Result<()> {
    for result in results.iter().filter(|r| r.success) {
        let Some(patch) = &result.patch else {
            continue;
        };

        match patch.operation {
            PatchOperation::Create => {
                println!("--- /dev/null");
                println!("+++ b/{}", patch.display_path);
                println!("@@ -0,0 +1,{} @@", patch.replace_content.lines().count());
                print_prefixed_lines('+', &patch.replace_content);
            }
            PatchOperation::Overwrite => {
                let old = read_target_text_with_encoding(&patch.file_path)
                    .map(|(text, _)| text)
                    .unwrap_or_default();
                println!("--- a/{}", patch.display_path);
                println!("+++ b/{}", patch.display_path);
                println!(
                    "@@ -1,{} +1,{} @@",
                    old.lines().count(),
                    patch.replace_content.lines().count()
                );
                print_prefixed_lines('-', &old);
                print_prefixed_lines('+', &patch.replace_content);
            }
            PatchOperation::Search => {
                let line = result.match_line.unwrap_or(1);
                println!("--- a/{}", patch.display_path);
                println!("+++ b/{}", patch.display_path);
                println!(
                    "@@ -{},{} +{},{} @@",
                    line,
                    patch.search_content.lines().count(),
                    line,
                    patch.replace_content.lines().count()
                );
                print_prefixed_lines('-', &patch.search_content);
                print_prefixed_lines('+', &patch.replace_content);
            }
        }
    }
    Ok(())
}

fn print_prefixed_lines(prefix: char, text: &str) {
    for line in text.lines() {
        println!("{prefix}{line}");
    }
}
