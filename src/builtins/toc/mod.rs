use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use glob::glob;
use serde::Serialize;

use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

#[derive(Args, Debug)]
#[command(
    long_about = "Pre-scan Markdown files and print their heading structure with 1-based line numbers.

Use this when an agent needs to navigate long Markdown documents before selecting exact line ranges to read. It is a discovery tool for Markdown headings, not a full Markdown parser and not a text search command. Use `rg` for search and `read-range` after choosing target lines.

Inputs may be Markdown files, directories, or glob patterns. Directories are searched recursively for .md files.",
    after_help = "Examples:
    asq md-toc README.md
    asq md-toc docs --depth 3
    asq md-toc \"docs/**/*.md\"
    asq --print json md-toc README.md docs --depth 2"
)]
pub struct TocArgs {
    #[arg(
        default_value = ".",
        help = "Markdown files, directories, or glob patterns"
    )]
    pub sources: Vec<String>,

    #[arg(
        long,
        default_value_t = 6,
        value_name = "N",
        help = "Maximum heading depth to include, clamped to 1..6"
    )]
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Heading {
    level: usize,
    text: String,
    line_num: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileToc {
    path: String,
    char_count: usize,
    line_count: usize,
    headings: Vec<Heading>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct TocData {
    files: Vec<FileToc>,
    count: usize,
    total_chars: usize,
    total_lines: usize,
    total_headings: usize,
}

pub fn run(args: TocArgs, ctx: &CommandContext) -> Result<u8> {
    let depth = args.depth.clamp(1, 6);
    let sources = if args.sources.is_empty() {
        vec![".".to_string()]
    } else {
        args.sources
    };
    let (files, missing) = resolve_sources(&sources)?;

    if files.is_empty() {
        if !missing.is_empty() {
            anyhow::bail!("No Markdown files found for: {}", missing.join(", "));
        }
        println!("No Markdown files found.");
        return Ok(0);
    }

    // Use first directory source as display base, if any
    let base = sources.iter().find_map(|s| {
        let p = PathBuf::from(s);
        if p.is_dir() { Some(p) } else { None }
    });

    let results = files
        .iter()
        .map(|path| analyze_file(path, depth, base.as_deref()))
        .collect::<Vec<_>>();

    let warnings: Vec<String> = missing
        .iter()
        .map(|s| format!("source not found: {s}"))
        .collect();

    match ctx.print {
        PrintMode::Json => {
            let data = TocData {
                count: results.len(),
                total_chars: results.iter().map(|r| r.char_count).sum(),
                total_lines: results.iter().map(|r| r.line_count).sum(),
                total_headings: results.iter().map(|r| r.headings.len()).sum(),
                files: results,
            };
            let payload = Envelope {
                ok: true,
                command: "toc",
                data,
                warnings,
                meta: serde_json::json!({ "depth": depth }),
            };
            output::print_json(&payload)?;
        }
        _ => {
            if results.len() > 1 {
                let total_chars: usize = results.iter().map(|r| r.char_count).sum();
                let total_lines: usize = results.iter().map(|r| r.line_count).sum();
                let total_headings: usize = results.iter().map(|r| r.headings.len()).sum();
                println!(
                    "Found {} files | total chars: {} | total lines: {} | headings: {}",
                    results.len(),
                    total_chars,
                    total_lines,
                    total_headings
                );
                println!();
            }

            for (idx, toc) in results.iter().enumerate() {
                if idx > 0 {
                    println!();
                }
                print!("{}", format_toc(toc));
            }
        }
    }

    Ok(0)
}

const GLOB_CHARS: &[char] = &['*', '?', '['];

fn resolve_sources(sources: &[String]) -> Result<(Vec<PathBuf>, Vec<String>)> {
    let mut files = Vec::new();
    let mut missing = Vec::new();

    for source in sources {
        let p = PathBuf::from(source);
        if p.is_dir() {
            let mut found = walkdir::WalkDir::new(&p)
                .sort_by_file_name()
                .into_iter()
                .filter_map(Result::ok)
                .map(|entry| entry.into_path())
                .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
                .collect::<Vec<_>>();
            found.sort();
            files.extend(found);
        } else if p.is_file() {
            files.push(p);
        } else if source.contains(GLOB_CHARS) {
            let matched: Vec<_> = glob(source)
                .with_context(|| format!("invalid glob pattern: {source}"))?
                .filter_map(Result::ok)
                .filter(|p| p.is_file())
                .collect();
            if matched.is_empty() {
                missing.push(source.clone());
            } else {
                files.extend(matched);
            }
        } else {
            missing.push(source.clone());
        }
    }

    Ok((files, missing))
}

fn analyze_file(path: &Path, max_depth: usize, base: Option<&Path>) -> FileToc {
    let display_path = base
        .and_then(|base| path.strip_prefix(base).ok())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            return FileToc {
                path: display_path,
                char_count: 0,
                line_count: 0,
                headings: vec![],
                error: Some(err.to_string()),
            };
        }
    };

    FileToc {
        path: display_path,
        char_count: content.chars().count(),
        line_count: content.lines().count(),
        headings: parse_headings(&content, max_depth),
        error: None,
    }
}

fn parse_headings(content: &str, max_depth: usize) -> Vec<Heading> {
    let mut headings = Vec::new();
    let mut in_fence = false;
    let mut fence_marker = "";

    for (idx, line) in content.lines().enumerate() {
        let stripped = line.trim();

        if stripped.starts_with("```") || stripped.starts_with("~~~") {
            let marker = &stripped[..3];
            if !in_fence {
                in_fence = true;
                fence_marker = marker;
            } else if marker == fence_marker {
                in_fence = false;
            }
            continue;
        }

        if in_fence || !line.starts_with('#') {
            continue;
        }

        let level = line.chars().take_while(|ch| *ch == '#').count();
        if level == 0 || level > 6 || level > max_depth {
            continue;
        }

        let text = line[level..].trim_start();
        if text.is_empty() {
            continue;
        }

        headings.push(Heading {
            level,
            text: text.to_string(),
            line_num: idx + 1,
        });
    }

    headings
}

fn format_toc(toc: &FileToc) -> String {
    let mut lines = Vec::new();
    lines.push(format!("=== {} ===", toc.path));
    lines.push(format!(
        "chars: {} | lines: {}",
        toc.char_count, toc.line_count
    ));

    if let Some(error) = &toc.error {
        lines.push(format!("ERROR: {error}"));
        return lines.join("\n") + "\n";
    }

    if toc.headings.is_empty() {
        lines.push("(no headings found)".into());
        return lines.join("\n") + "\n";
    }

    lines.push(String::new());
    let min_level = toc.headings.iter().map(|h| h.level).min().unwrap_or(1);

    for heading in &toc.headings {
        let indent = "  ".repeat(heading.level - min_level);
        let hashes = "#".repeat(heading.level);
        lines.push(format!(
            "L{:<5} {}{} {}",
            heading.line_num, indent, hashes, heading.text
        ));
    }

    lines.join("\n") + "\n"
}
