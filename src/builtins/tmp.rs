use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;
use clap::Args;
use serde::Serialize;

use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

#[derive(Args, Debug)]
#[command(
    long_about = "Create a temporary file or directory under a temp root.\n\nUseful when an agent or script needs a quick scratch file or directory that lives outside the current workspace. By default a timestamp prefix is added to the name to avoid collisions.",
    after_help = "Examples:\n  squire tmp note\n  squire tmp --dir logs\n  squire tmp --open scratch.py\n  squire tmp --no-time-prefix todo.md\n  squire tmp --dir"
)]
pub struct TmpArgs {
    #[arg(
        help = "Name of the temporary file or directory; a random 5-char name is used if omitted"
    )]
    pub name: Option<String>,

    #[arg(
        short = 'f',
        long,
        conflicts_with = "dir",
        help = "Force creating a file"
    )]
    pub file: bool,

    #[arg(
        short = 'd',
        long,
        conflicts_with = "file",
        help = "Force creating a directory"
    )]
    pub dir: bool,

    #[arg(
        short = 'r',
        long,
        value_name = "DIR",
        help = "Root directory; defaults to <SYSTEM_TEMP_DIR>/asq-temp"
    )]
    pub root: Option<PathBuf>,

    #[arg(short = 'o', long, help = "Open the created file or directory")]
    pub open: bool,

    #[arg(
        long = "no-time-prefix",
        visible_alias = "ntp",
        help = "Disable the default timestamp prefix"
    )]
    pub no_time_prefix: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    File,
    Dir,
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Kind::File => write!(f, "file"),
            Kind::Dir => write!(f, "dir"),
        }
    }
}

#[derive(Debug, Serialize)]
struct TmpData {
    path: String,
    kind: String,
    root: String,
}

pub fn run(args: TmpArgs, ctx: &CommandContext) -> Result<u8> {
    let root = match args.root {
        Some(r) => r,
        None => std::env::temp_dir().join("asq-temp"),
    };

    fs::create_dir_all(&root)
        .with_context(|| format!("failed to create root directory {}", root.display()))?;

    let name = args.name.unwrap_or_else(|| {
        uuid::Uuid::new_v4()
            .as_simple()
            .to_string()
            .chars()
            .take(5)
            .collect()
    });

    let (relative_target, kind) = infer_target(&name, args.file, args.dir);
    let mut target = root.join(&relative_target);

    if !args.no_time_prefix {
        let prefix = Local::now().format("%Y-%m-%dT%H-%M_").to_string();
        target = apply_prefix(target, &prefix);
    }

    create_target(&target, kind)
        .with_context(|| format!("failed to create {}", target.display()))?;

    if args.open {
        open_target(&target, kind)?;
    }

    match ctx.print {
        PrintMode::Json => {
            let data = TmpData {
                path: target.to_string_lossy().to_string(),
                kind: kind.to_string(),
                root: root.to_string_lossy().to_string(),
            };
            let payload = Envelope {
                ok: true,
                command: "tmp",
                data,
                warnings: vec![],
                meta: serde_json::json!({}),
            };
            output::print_json(&payload)?;
        }
        _ => {
            println!("{}", target.display());
        }
    }

    Ok(0)
}

fn infer_target(name: &str, force_file: bool, force_dir: bool) -> (PathBuf, Kind) {
    let trimmed = name.trim_end_matches('/');

    if force_file {
        return (PathBuf::from(trimmed), Kind::File);
    }
    if force_dir {
        return (PathBuf::from(trimmed), Kind::Dir);
    }
    if name.ends_with('/') {
        return (PathBuf::from(trimmed), Kind::Dir);
    }
    if Path::new(name).extension().is_some() {
        return (PathBuf::from(name), Kind::File);
    }

    (PathBuf::from(format!("{name}.md")), Kind::File)
}

fn apply_prefix(path: PathBuf, prefix: &str) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    match path.file_name().and_then(|s| s.to_str()) {
        Some(name) => parent.join(format!("{prefix}{name}")),
        None => path,
    }
}

fn create_target(path: &Path, kind: Kind) -> Result<()> {
    match kind {
        Kind::File => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create parent directory {}", parent.display())
                })?;
            }
            if !path.exists() {
                fs::File::create(path)
                    .with_context(|| format!("failed to create file {}", path.display()))?;
            }
        }
        Kind::Dir => {
            if !path.exists() {
                fs::create_dir_all(path)
                    .with_context(|| format!("failed to create directory {}", path.display()))?;
            }
        }
    }
    Ok(())
}

fn open_target(path: &Path, kind: Kind) -> Result<()> {
    match kind {
        Kind::File => {
            if let Some(editor) = std::env::var_os("EDITOR").or_else(|| std::env::var_os("VISUAL"))
            {
                std::process::Command::new(&editor)
                    .arg(path)
                    .spawn()
                    .with_context(|| {
                        format!(
                            "failed to open file {} with editor {:?}",
                            path.display(),
                            editor
                        )
                    })?;
            } else {
                open::that(path)
                    .with_context(|| format!("failed to open file {}", path.display()))?;
            }
        }
        Kind::Dir => {
            open::that(path)
                .with_context(|| format!("failed to open directory {}", path.display()))?;
        }
    }
    Ok(())
}
