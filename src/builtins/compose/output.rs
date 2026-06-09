use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::Local;
use serde::Serialize;
use uuid::Uuid;

use crate::runtime::output::{self, Envelope, PrintMode};

use super::model::{ComposeError, ComposeStatus, OutputInfo};
use super::text::utf8_bytes;

#[derive(Debug, Clone)]
pub enum OutputTarget {
    Temp,
    Stdout,
    File(PathBuf),
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope<'a> {
    ok: bool,
    command: &'static str,
    error: &'a ComposeError,
    warnings: Vec<String>,
    meta: serde_json::Value,
}

pub fn resolve_target(stdout: bool, output: Option<PathBuf>) -> Result<OutputTarget> {
    match (stdout, output) {
        (true, Some(_)) => bail!("--stdout and --output are mutually exclusive"),
        (true, None) => Ok(OutputTarget::Stdout),
        (false, Some(path)) => Ok(OutputTarget::File(path)),
        (false, None) => Ok(OutputTarget::Temp),
    }
}

pub fn write_rendered(
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
            let path = temp_output_path()?;
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

pub fn print_success(status: &ComposeStatus, print: PrintMode) -> Result<()> {
    if status.output.is_none() {
        return Ok(());
    }

    match print {
        PrintMode::Json => {
            let payload = Envelope {
                ok: true,
                command: "compose",
                data: status,
                warnings: vec![],
                meta: serde_json::json!({}),
            };
            output::print_json(&payload)?;
        }
        _ => {
            if let Some(output) = &status.output {
                println!("output: {}", output.path);
            }
        }
    }
    Ok(())
}

pub fn print_check_ok(print: PrintMode, sources: usize) -> Result<()> {
    match print {
        PrintMode::Json => {
            let payload = Envelope {
                ok: true,
                command: "compose",
                data: serde_json::json!({ "valid": true, "sources": sources }),
                warnings: vec![],
                meta: serde_json::json!({}),
            };
            output::print_json(&payload)?;
        }
        _ => println!("ok: template valid ({sources} sources)"),
    }
    Ok(())
}

pub fn print_error(error: &ComposeError, print: PrintMode) -> Result<()> {
    match print {
        PrintMode::Json => {
            let payload = ErrorEnvelope {
                ok: false,
                command: "compose",
                error,
                warnings: vec![],
                meta: serde_json::json!({}),
            };
            eprintln!("{}", serde_json::to_string_pretty(&payload)?);
        }
        _ => {
            if let Some(location) = &error.location {
                eprintln!(
                    "error: {} at {}:{}: {}",
                    error.code, location.line, location.column, error.message
                );
            } else {
                eprintln!("error: {}: {}", error.code, error.message);
            }
        }
    }
    Ok(())
}

fn temp_output_path() -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("agent-temp");
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let timestamp = Local::now().format("%Y%m%dT%H%M%S");
    Ok(dir.join(format!("asq-compose-{timestamp}-{}.md", Uuid::new_v4())))
}

fn atomic_write_utf8(path: &Path, text: &str, overwrite: bool) -> Result<()> {
    if path.exists() && !overwrite {
        bail!("output file exists: {} (pass --overwrite)", path.display());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temp file in {}", parent.display()))?;
    tmp.write_all(&utf8_bytes(text))?;
    tmp.persist(path)
        .map_err(|err| anyhow::anyhow!("failed to persist {}: {}", path.display(), err.error))?;
    Ok(())
}
