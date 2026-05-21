mod io;
mod match_apply;
mod model;
mod output;
mod parse;
mod text;

use anyhow::{bail, Result};
use clap::Args;

use crate::cli::CommandContext;
use crate::runtime::input;
use crate::runtime::output::PrintMode;

pub use match_apply::{apply_parsed_patches, apply_patches};
pub use model::{PatchApplyResult, PatchBlock, PatchOperation};
pub use parse::parse_patches;

const PATCH_PROMPT: &str = r#"# Squire patch-edit format

1) Targeted edit by SEARCH/REPLACE

```patch
# <path>[:<range>]
<<<<<<< SEARCH
old content
=======
new content
>>>>>>> REPLACE
```

Line ranges are 1-based and optional:
  L10-L25
  L10-
  -L25
  10-20

2) Create new file

```patch
# <path>
<<<<<<< CREATE
=======
new file content
>>>>>>> REPLACE
```

3) Full overwrite

```patch
# <path>
<<<<<<< OVERWRITE
=======
full replacement content
>>>>>>> REPLACE
```

Rules:
- Markers must appear alone on their own lines.
- CREATE and OVERWRITE upper blocks must be whitespace-only.
- Same-file SEARCH patches are matched against original file content first.
- Use --dry-run before writing.
"#;

#[derive(Args, Debug)]
#[command(long_about = "Apply SEARCH/REPLACE patch blocks. The patch argument supports literal text, @stdin, @file:path, and @env:NAME.")]
pub struct PatchEditArgs {
    #[arg(help = "Patch content or input source: literal, @stdin, @file:path, @env:NAME")]
    pub patch: Option<String>,

    #[arg(short = 'f', long = "file", value_name = "PATH", help = "Read patch text from a file")]
    pub file: Option<std::path::PathBuf>,

    #[arg(long, help = "Read patch text from stdin")]
    pub stdin: bool,

    #[arg(long, help = "Validate without modifying files")]
    pub dry_run: bool,

    #[arg(short = 'y', long, help = "Required for non-dry-run writes")]
    pub yes: bool,

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
    if source_count > 1 {
        bail!("use exactly one patch source: positional PATCH, --file, or --stdin");
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

    let results = apply_patches(&patch_text, &ctx.cwd, args.dry_run);
    let all_success = results.iter().all(|r| r.success);

    match ctx.print {
        PrintMode::Json => output::print_json(&results, args.dry_run)?,
        _ => output::print_compact(&results, args.dry_run),
    }

    Ok(if all_success { 0 } else { 1 })
}
