use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use ignore::WalkBuilder;
use serde::Serialize;

use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

const TREE_PROMPT: &str = r#"# Squire file-tree guide

`asq file-tree` displays a compact project directory tree for codebase orientation.

## Flags

| Flag | Purpose |
|------|---------|
| `-d N` / `--depth N` | Max tree depth (default: unlimited) |
| `--dirs-only` | Directories only, omit files |
| `--show-size` | Show file sizes |
| `--detail` | Show line/char counts for text files |
| `--no-gitignore` | Include gitignored files |
| `-o PATH` | Write output to file |

## CLI Usage (for AGENT)

```bash
# Quick overview (2 levels deep)
asq file-tree . -d 2

# High-level structure only
asq file-tree . --dirs-only

# Subdirectory with file details
asq file-tree src --detail

# Multiple paths
asq file-tree src tests docs -d 3

# Include hidden/ignored files
asq file-tree . --no-gitignore

# Save to file
asq file-tree . -d 3 -o tree.md
```

## Typical Workflow

1. **Orient**: `asq file-tree . -d 2` — understand repo layout
2. **Drill down**: `asq file-tree src/module --detail` — see file sizes/lines
3. **Read**: `asq read-range <file> -r <range>` — read specific content

## Tips

- Default respects `.gitignore` and skips `.git`, `node_modules`, `__pycache__`, etc.
- Use `--depth` to keep context small; AGENT context is precious.
- `--detail` adds line/char counts — useful for deciding what to read next.
- JSON mode (`--print json`) returns structured data with stats.
"#;

#[derive(Args, Debug)]
#[command(
    long_about = "Display a compact project directory tree for orientation before reading files.\n\nUse this when an agent needs to understand repository layout, choose likely files, or inspect a subdirectory without dumping file contents. By default it respects nested .gitignore files and hides common noise such as .git, node_modules, __pycache__, and cache directories.\n\nUse `--depth` to keep context small, `--dirs-only` for high-level structure, and `--detail` only when line/character counts are useful for deciding what to read next.",
    after_help = "Examples:\n  squire file-tree . -d 2\n  squire file-tree src tests --dirs-only\n  squire file-tree . --show-size\n  squire file-tree docs --detail\n  squire --print json file-tree src -d 3"
)]
pub struct TreeArgs {
    #[arg(default_value = ".", help = "Directories to display")]
    pub paths: Vec<PathBuf>,

    #[arg(
        short = 'd',
        long = "depth",
        value_name = "N",
        help = "Maximum tree depth to display"
    )]
    pub depth: Option<usize>,

    #[arg(
        long,
        help = "Include files normally hidden by .gitignore and built-in skip rules"
    )]
    pub no_gitignore: bool,

    #[arg(long, help = "Show directory structure only, omitting files")]
    pub dirs_only: bool,

    #[arg(long = "show-size", help = "Show file sizes in compact output")]
    pub show_size: bool,

    #[arg(
        long,
        help = "Show UTF-8 text file line/character counts for read planning"
    )]
    pub detail: bool,

    #[arg(
        short = 'o',
        long,
        value_name = "PATH",
        help = "Write compact output to a file"
    )]
    pub output: Option<PathBuf>,

    #[arg(long, help = "Print the agent-facing usage guide")]
    pub prompt: bool,
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

const ALWAYS_SKIP: &[&str] = &[
    ".git",
    "__pycache__",
    "node_modules",
    ".pytest_cache",
    ".mypy_cache",
];

pub fn run(args: TreeArgs, ctx: &CommandContext) -> Result<u8> {
    if args.prompt {
        println!("{TREE_PROMPT}");
        return Ok(0);
    }

    let paths = if args.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        args.paths.clone()
    };

    for path in &paths {
        if !path.exists() {
            anyhow::bail!("path does not exist: {}", path.display());
        }
        if !path.is_dir() {
            anyhow::bail!("path is not a directory: {}", path.display());
        }
    }

    let mut all_outputs: Vec<(String, Vec<String>, Stats)> = Vec::new();

    for path in &paths {
        let mut tree = build_tree(path, &args)?;
        mark_omitted_child_counts(&mut tree);
        let stats = compute_stats(&tree);
        let mut lines = Vec::new();
        let root_name = path.file_name().and_then(|s| s.to_str()).unwrap_or(".");
        lines.push(format!("{root_name}/"));
        render_tree_node(&tree, &args, "", &mut lines);
        all_outputs.push((path.display().to_string(), lines, stats));
    }

    match ctx.print {
        PrintMode::Json => print_json_output(&all_outputs)?,
        _ => print_compact_output(&all_outputs, args.output.as_deref())?,
    }

    Ok(0)
}

/// A node in the directory tree. Children are sorted: dirs first, then alphabetical.
#[derive(Debug)]
struct TreeNode {
    name: String,
    full_path: PathBuf,
    is_dir: bool,
    size: u64,
    children: Vec<TreeNode>,
    /// When `--detail`: Some(N) = direct filesystem entries omitted from tree output.
    omitted_child_count: Option<usize>,
}

/// Build a tree structure using `ignore::WalkBuilder` which handles nested .gitignore.
fn build_tree(root: &Path, args: &TreeArgs) -> Result<TreeNode> {
    let mut walker = WalkBuilder::new(root);
    walker
        .hidden(false)
        .git_ignore(!args.no_gitignore)
        .git_global(!args.no_gitignore)
        .git_exclude(!args.no_gitignore)
        .sort_by_file_name(sort_entry_name);

    if let Some(max_depth) = args.depth {
        // +1 because WalkBuilder depth includes the root itself
        walker.max_depth(Some(max_depth + 1));
    }

    // Custom filter for ALWAYS_SKIP entries when gitignore is active
    let use_skip = !args.no_gitignore;
    walker.filter_entry(move |entry| {
        if !use_skip {
            return true;
        }
        let name = entry.file_name().to_str().unwrap_or("");
        !ALWAYS_SKIP.contains(&name)
    });

    // Collect all entries into a parent→children map
    let mut children_map: BTreeMap<PathBuf, Vec<(PathBuf, bool, u64)>> = BTreeMap::new();
    let canon_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    for entry in walker.build() {
        let entry = entry.with_context(|| "failed to walk directory")?;
        let path = entry.path().to_path_buf();
        let canon = path.canonicalize().unwrap_or_else(|_| path.clone());

        if canon == canon_root {
            continue; // skip root itself
        }

        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let size = if is_dir {
            0
        } else {
            entry.metadata().map(|m| m.len()).unwrap_or(0)
        };

        let parent = path.parent().unwrap_or(root).to_path_buf();
        children_map
            .entry(parent)
            .or_default()
            .push((path, is_dir, size));
    }

    // Recursively build tree from the map
    fn build_node(
        path: &Path,
        is_dir: bool,
        size: u64,
        children_map: &BTreeMap<PathBuf, Vec<(PathBuf, bool, u64)>>,
    ) -> TreeNode {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(".")
            .to_string();

        let children = if is_dir {
            children_map
                .get(path)
                .map(|entries| {
                    let mut nodes: Vec<TreeNode> = entries
                        .iter()
                        .map(|(p, d, s)| build_node(p, *d, *s, children_map))
                        .collect();
                    // Sort: dirs first, then alphabetical (case-insensitive)
                    nodes.sort_by(|a, b| {
                        a.is_dir
                            .cmp(&b.is_dir)
                            .reverse()
                            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                    });
                    nodes
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        TreeNode {
            name,
            full_path: path.to_path_buf(),
            is_dir,
            size,
            children,
            omitted_child_count: None,
        }
    }

    // Build a virtual root node
    let root_children = children_map
        .get(root)
        .map(|entries| {
            let mut nodes: Vec<TreeNode> = entries
                .iter()
                .map(|(p, d, s)| build_node(p, *d, *s, &children_map))
                .collect();
            nodes.sort_by(|a, b| {
                a.is_dir
                    .cmp(&b.is_dir)
                    .reverse()
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            nodes
        })
        .unwrap_or_default();

    Ok(TreeNode {
        name: root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(".")
            .to_string(),
        full_path: root.to_path_buf(),
        is_dir: true,
        size: 0,
        children: root_children,
        omitted_child_count: None,
    })
}

fn sort_entry_name(a: &OsStr, b: &OsStr) -> std::cmp::Ordering {
    let a_s = a.to_string_lossy().to_lowercase();
    let b_s = b.to_string_lossy().to_lowercase();
    a_s.cmp(&b_s)
}

fn compute_stats(node: &TreeNode) -> Stats {
    let mut stats = Stats {
        files: 0,
        directories: 0,
        total_size: 0,
    };
    count_stats(node, &mut stats);
    stats
}

fn count_stats(node: &TreeNode, stats: &mut Stats) {
    for child in &node.children {
        if child.is_dir {
            stats.directories += 1;
            count_stats(child, stats);
        } else {
            stats.files += 1;
            stats.total_size += child.size;
        }
    }
}

/// For leaf directories in the rendered tree, record direct filesystem entries
/// omitted by depth limits, ignore filters, or built-in skip rules.
fn mark_omitted_child_counts(node: &mut TreeNode) {
    if !node.is_dir {
        return;
    }
    if node.children.is_empty() {
        if let Ok(entries) = std::fs::read_dir(&node.full_path) {
            let count = entries.count();
            if count > 0 {
                node.omitted_child_count = Some(count);
            }
        }
    } else {
        for child in &mut node.children {
            mark_omitted_child_counts(child);
        }
    }
}

fn render_tree_node(node: &TreeNode, args: &TreeArgs, prefix: &str, out: &mut Vec<String>) {
    let children: Vec<&TreeNode> = if args.dirs_only {
        node.children.iter().filter(|c| c.is_dir).collect()
    } else {
        node.children.iter().collect()
    };

    for (idx, child) in children.iter().enumerate() {
        let is_last = idx + 1 == children.len();
        let connector = if is_last {
            "\u{2514}\u{2500}\u{2500} "
        } else {
            "\u{251c}\u{2500}\u{2500} "
        };
        let mut label = child.name.clone();

        if child.is_dir {
            label.push('/');
            if args.detail {
                if let Some(count) = child.omitted_child_count {
                    label.push_str(&format!(" ({count} omitted items)"));
                }
            }
        } else if args.detail {
            let (lines, chars) = count_text_stats(&child.full_path);
            if lines > 0 {
                label.push_str(&format!(" ({lines} lines, {chars} chars)"));
            } else {
                label.push_str(&format!(" ({})", format_size(child.size as f64)));
            }
        } else if args.show_size {
            label.push_str(&format!(" ({})", format_size(child.size as f64)));
        }

        out.push(format!("{prefix}{connector}{label}"));

        if child.is_dir {
            let child_prefix = format!("{prefix}{}", if is_last { "    " } else { "\u{2502}   " });
            render_tree_node(child, args, &child_prefix, out);
        }
    }
}

fn print_json_output(outputs: &[(String, Vec<String>, Stats)]) -> Result<()> {
    if outputs.len() == 1 {
        let (root, lines, stats) = &outputs[0];
        let data = TreeData {
            root: root.clone(),
            lines: lines.clone(),
            stats: stats.clone(),
        };
        let payload = Envelope {
            ok: true,
            command: "tree",
            data,
            warnings: vec![],
            meta: serde_json::json!({}),
        };
        output::print_json(&payload)?;
    } else {
        let trees: Vec<TreeData> = outputs
            .iter()
            .map(|(root, lines, stats)| TreeData {
                root: root.clone(),
                lines: lines.clone(),
                stats: stats.clone(),
            })
            .collect();
        let payload = Envelope {
            ok: true,
            command: "tree",
            data: trees,
            warnings: vec![],
            meta: serde_json::json!({}),
        };
        output::print_json(&payload)?;
    }
    Ok(())
}

fn print_compact_output(
    outputs: &[(String, Vec<String>, Stats)],
    out_file: Option<&Path>,
) -> Result<()> {
    let mut text = String::new();
    for (idx, (_root, lines, stats)) in outputs.iter().enumerate() {
        if idx > 0 {
            text.push('\n');
        }
        text.push_str(&lines.join("\n"));
        text.push_str(&format!(
            "\n\nFiles: {} | Directories: {} | Total size: {}\n",
            stats.files,
            stats.directories,
            format_size(stats.total_size as f64)
        ));
    }

    if let Some(path) = out_file {
        std::fs::write(path, &text)
            .with_context(|| format!("failed to write {}", path.display()))?;
        println!("[OK] Saved to: {}", path.display());
    } else {
        print!("{text}");
    }
    Ok(())
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

fn count_text_stats(path: &Path) -> (usize, usize) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return (0, 0);
    };
    let lines = content.matches('\n').count()
        + usize::from(!content.is_empty() && !content.ends_with('\n'));
    (lines, content.chars().count())
}
