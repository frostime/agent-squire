//! `data-toc` built-in: bounded structural preview for JSON/JSONL/YAML files.
//!
//! The public surface is intentionally tiny: [`DataTocArgs`] and [`run`].
//! Everything else lives in sibling modules grouped by concern.

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

mod analyze;
mod render;
mod types;
mod util;

use types::DataFormat;
pub use types::DataTocArgs;

const DATA_TOC_PROMPT: &str = r#"# Squire data-toc guide

`asq data-toc` summarizes JSON, JSONL, and YAML structure before an agent reads raw data content.

## When to use

- Unknown JSON / JSONL / YAML files where structure matters more than values.
- Large arrays with repeated objects.
- JSONL logs or event streams that may contain multiple record shapes.
- YAML configuration files when `yq` is available.
- Before choosing precise `jq`, `sed`, or `read-range` follow-up reads.

## Commands

```bash
asq data-toc result.json
asq data-toc logs.jsonl --format jsonl
asq data-toc compose.yaml --format yaml
asq data-toc result.json --budget large
asq data-toc result.json --examples
asq --print json data-toc result.json
```

## Output interpretation

- `[]` means array indexes are collapsed.
- `?` means observed in only part of the sample.
- `complete=false` means budget limits or sampling affected the scan.
- JSONL record groups are approximate structural clusters.
- Values are hidden by default; `--examples` prints limited truncated/redacted examples.

## Follow-up reads

```bash
jq '.runs[0:5]' result.json
sed -n '37p' logs.jsonl | jq .
```
"#;

pub fn run(args: DataTocArgs, ctx: &CommandContext) -> Result<u8> {
    if args.prompt {
        println!("{DATA_TOC_PROMPT}");
        return Ok(0);
    }

    let path = args
        .path
        .as_deref()
        .context("missing path; use --prompt for the agent-facing guide")?;
    if !path.is_file() {
        bail!("path is not a file: {}", path.display());
    }

    let format = resolve_format(path, args.format)?;
    let profile = args.budget.profile();
    let (data, warnings) = match format {
        DataFormat::Auto => unreachable!("auto format should be resolved"),
        DataFormat::Json => analyze::analyze_json(path, profile, args.examples)?,
        DataFormat::Jsonl => analyze::analyze_jsonl(path, profile, args.examples)?,
        DataFormat::Yaml => analyze::analyze_yaml(path, profile, args.examples)?,
    };

    match ctx.print {
        PrintMode::Json => {
            let payload = Envelope {
                ok: true,
                command: "data-toc",
                data,
                warnings,
                meta: serde_json::json!({
                    "budget": args.budget,
                    "schema_version": 1,
                }),
            };
            output::print_json(&payload)?;
        }
        _ => render::print_compact(&data, &warnings, args.budget),
    }

    Ok(0)
}

fn resolve_format(path: &Path, requested: DataFormat) -> Result<DataFormat> {
    if requested != DataFormat::Auto {
        return Ok(requested);
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "json" => Ok(DataFormat::Json),
        "jsonl" | "ndjson" => Ok(DataFormat::Jsonl),
        "yaml" | "yml" => Ok(DataFormat::Yaml),
        _ => bail!(
            "format could not be detected; pass --format json, --format jsonl, or --format yaml"
        ),
    }
}
