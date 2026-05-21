use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::Serialize;

use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

const ALWAYS_SKIP: &[&str] = &[".git", "__pycache__", "node_modules", ".pytest_cache", ".mypy_cache"];
const FALLBACK_SKIP: &[&str] = &[
    ".git", "__pycache__", "node_modules", ".pytest_cache", ".mypy_cache",
    ".ruff_cache", ".venv", "venv", ".env", "dist", ".tox", ".eggs",
];

#[derive(Args, Debug)]
#[command(long_about = "Display a directory tree. Respects .gitignore when possible and always hides common noise such as .git and node_modules unless --no-gitignore is used.")]
pub struct TreeArgs {
    #[arg(default_value = ".", help = "Directory to display")]
    pub path: PathBuf,

    #[arg(short = 'd', long = "depth", value_name = "N", help = "Maximum depth")]
    pub depth: Option<usize>,

    #[arg(long, help = "Do not apply .gitignore or built-in skip rules")]
    pub no_gitignore: bool,

    #[arg(long, help = "Show only directories")]
    pub dirs_only: bool,

    #[arg(long = "show-size", help = "Show file sizes")]
    pub show_size: bool,

    #[arg(long, help = "Show line/character counts for UTF-8 text files")]
    pub detail: bool,

    #[arg(short = 'o', long, value_name = "PATH", help = "Write compact output to a file")]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct TreeOptions {
    depth: Option<usize>,
    no_gitignore: bool,
    dirs_only: bool,
    show_size: bool,
    detail: bool,
}

#[derive(Debug, Serialize)]
struct TreeData {
    root: String,
    lines: Vec<String>,
    stats: Stats,
}

#[derive(Debug, Serialize, Clone)]
struct Stats {
    files: usize,
    directories: usize,
    total_size: u64,
}

pub fn run(args: TreeArgs, ctx: &CommandContext) -> Result<u8> {
    let path = args.path;
    if !path.exists() {
        anyhow::bail!("path does not exist: {}", path.display());
    }
    if !path.is_dir() {
        anyhow::bail!("path is not a directory: {}", path.display());
    }

    let opts = TreeOptions {
        depth: args.depth,
        no_gitignore: args.no_gitignore,
        dirs_only: args.dirs_only,
        show_size: args.show_size,
        detail: args.detail,
    };

    let filter = IgnoreFilter::new(&path, opts.no_gitignore);
    let stats = collect_stats(&path, &opts, &filter)?;
    let mut lines = Vec::new();
    lines.push(format!("{}/", path.file_name().and_then(|s| s.to_str()).unwrap_or(".")));
    render_children(&path, &opts, &filter, 0, "", &mut lines)?;

    match ctx.print {
        PrintMode::Json => {
            let data = TreeData {
                root: path.display().to_string(),
                lines,
                stats,
            };
            let payload = Envelope {
                ok: true,
                command: "tree",
                data,
                warnings: vec![],
                meta: serde_json::json!({}),
            };
            output::print_json(&payload)?;
        }
        _ => {
            let mut text = lines.join("\n");
            text.push_str("\n\n");
            text.push_str(&format!(
                "Files: {} | Directories: {} | Total size: {}\n",
                stats.files,
                stats.directories,
                format_size(stats.total_size as f64)
            ));
            if let Some(path) = args.output {
                fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))?;
                println!("[OK] Saved to: {}", path.display());
            } else {
                print!("{text}");
            }
        }
    }

    Ok(0)
}

struct IgnoreFilter {
    no_gitignore: bool,
    gitignore: Option<Gitignore>,
}

impl IgnoreFilter {
    fn new(start: &Path, no_gitignore: bool) -> Self {
        if no_gitignore {
            return Self { no_gitignore, gitignore: None };
        }

        let root = find_project_root(start).unwrap_or_else(|| start.to_path_buf());
        let mut builder = GitignoreBuilder::new(&root);
        let ignore_path = root.join(".gitignore");
        if ignore_path.exists() {
            let _ = builder.add(ignore_path);
        }

        let gitignore = builder.build().ok();
        Self { no_gitignore, gitignore }
    }

    fn should_skip(&self, path: &Path) -> bool {
        if self.no_gitignore {
            return false;
        }

        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if ALWAYS_SKIP.contains(&name) {
            return true;
        }

        if let Some(gi) = &self.gitignore {
            if gi.matched_path_or_any_parents(path, path.is_dir()).is_ignore() {
                return true;
            }
        } else if path.is_dir() && FALLBACK_SKIP.contains(&name) {
            return true;
        }

        false
    }
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.canonicalize().ok()?;
    for _ in 0..100 {
        if current.join(".gitignore").exists() || current.join(".git").is_dir() {
            return Some(current);
        }
        let parent = current.parent()?.to_path_buf();
        if parent == current {
            break;
        }
        current = parent;
    }
    None
}

fn sorted_children(path: &Path, filter: &IgnoreFilter) -> Result<Vec<PathBuf>> {
    let mut entries = fs::read_dir(path)
        .with_context(|| format!("failed to read {}", path.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| !filter.should_skip(path))
        .collect::<Vec<_>>();

    entries.sort_by_key(|p| {
        (
            !p.is_dir(),
            p.file_name()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default(),
        )
    });

    Ok(entries)
}

fn render_children(
    root: &Path,
    opts: &TreeOptions,
    filter: &IgnoreFilter,
    depth: usize,
    prefix: &str,
    out: &mut Vec<String>,
) -> Result<()> {
    if opts.depth.is_some_and(|max| depth >= max) {
        return Ok(());
    }

    let entries = sorted_children(root, filter)?;
    let visible = entries
        .into_iter()
        .filter(|p| !opts.dirs_only || p.is_dir())
        .collect::<Vec<_>>();

    for (idx, path) in visible.iter().enumerate() {
        let is_last = idx + 1 == visible.len();
        let connector = if is_last { "└── " } else { "├── " };
        let mut label = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();

        if path.is_dir() {
            label.push('/');
        } else if opts.detail {
            let (lines, chars) = count_text_stats(path);
            if lines > 0 {
                label.push_str(&format!(" ({lines} lines, {chars} chars)"));
            } else if let Ok(meta) = fs::metadata(path) {
                label.push_str(&format!(" ({})", format_size(meta.len() as f64)));
            }
        } else if opts.show_size {
            if let Ok(meta) = fs::metadata(path) {
                label.push_str(&format!(" ({})", format_size(meta.len() as f64)));
            }
        }

        out.push(format!("{prefix}{connector}{label}"));

        if path.is_dir() {
            let child_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
            render_children(path, opts, filter, depth + 1, &child_prefix, out)?;
        }
    }

    Ok(())
}

fn collect_stats(root: &Path, opts: &TreeOptions, filter: &IgnoreFilter) -> Result<Stats> {
    let mut stats = Stats { files: 0, directories: 0, total_size: 0 };
    collect_stats_inner(root, opts, filter, &mut stats)?;
    Ok(stats)
}

fn collect_stats_inner(root: &Path, opts: &TreeOptions, filter: &IgnoreFilter, stats: &mut Stats) -> Result<()> {
    for path in sorted_children(root, filter)? {
        if path.is_dir() {
            stats.directories += 1;
            collect_stats_inner(&path, opts, filter, stats)?;
        } else if !opts.dirs_only {
            stats.files += 1;
            if let Ok(meta) = fs::metadata(&path) {
                stats.total_size += meta.len();
            }
        }
    }
    Ok(())
}

fn count_text_stats(path: &Path) -> (usize, usize) {
    let Ok(content) = fs::read_to_string(path) else {
        return (0, 0);
    };
    let lines = content.matches('\n').count() + usize::from(!content.is_empty() && !content.ends_with('\n'));
    (lines, content.chars().count())
}

fn format_size(mut size: f64) -> String {
    for unit in ["B", "KB", "MB", "GB"] {
        if size < 1024.0 {
            return format!("{size:.1}{unit}");
        }
        size /= 1024.0;
    }
    format!("{size:.1}TB")
}
