use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use glob::glob;
use ignore::WalkBuilder;
use serde::Serialize;

use crate::builtins::md_links::graph;
use crate::builtins::md_links::model::{LinkKind, SourceFile, TargetType};
use crate::builtins::md_links::sources::display_path;
use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

#[derive(Args, Debug)]
#[command(
    long_about = r#"Find Markdown files that link to one or more target files.

Positional pages are the focus pages whose incoming links should be reported. Corpus files are selected with --from and default to the effective current working directory. Directory corpus walks respect .gitignore and built-in skip rules by default."#,
    after_help = r#"Examples:
    asq md-backlinks notes/foo.md
    asq md-backlinks notes/foo.md --from docs --from README.md
    asq --print json md-backlinks notes/foo.md --from ."#
)]
pub struct MdBacklinksArgs {
    #[arg(
        required = true,
        help = "Focus pages whose backlinks should be reported"
    )]
    pub pages: Vec<String>,

    #[arg(
        long = "from",
        value_name = "PATH",
        help = "Markdown files, directories, or glob patterns to scan"
    )]
    pub from: Vec<String>,

    #[arg(
        long,
        help = "Include files normally hidden by .gitignore and built-in skip rules"
    )]
    pub no_gitignore: bool,
}

#[derive(Debug, Serialize)]
struct MdBacklinksData {
    pages: Vec<MdBacklinksPage>,
    focus_count: usize,
    corpus_files: usize,
    total_backlinks: usize,
}

#[derive(Debug, Serialize)]
struct MdBacklinksPage {
    path: String,
    exists: bool,
    backlinks: Vec<MdBacklink>,
}

#[derive(Debug, Serialize)]
struct MdBacklink {
    source: String,
    line_num: usize,
    kind: LinkKind,
    raw: String,
}

#[derive(Debug, Clone)]
struct FocusPage {
    path: String,
    exists: bool,
}

const GLOB_CHARS: &[char] = &['*', '?', '['];
const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown"];
const ALWAYS_SKIP: &[&str] = &[
    ".git",
    "__pycache__",
    "node_modules",
    ".pytest_cache",
    ".mypy_cache",
];

pub fn run(args: MdBacklinksArgs, ctx: &CommandContext) -> Result<u8> {
    let root = ctx.cwd.clone();
    let from = if args.from.is_empty() {
        vec![".".to_string()]
    } else {
        args.from.clone()
    };

    let focus_pages = dedupe_focus_pages(
        args.pages
            .iter()
            .map(|page| normalize_focus_page(page, &root)),
    );
    let focus_index = focus_pages
        .iter()
        .enumerate()
        .map(|(idx, page)| (normalized_path_key(&page.path), idx))
        .collect::<BTreeMap<_, _>>();

    let (corpus_files, mut warnings) = discover_corpus(&from, &root, !args.no_gitignore)?;
    if corpus_files.is_empty() && !warnings.is_empty() {
        anyhow::bail!("No Markdown files found for: {}", from.join(", "));
    }

    let mut pages = focus_pages
        .iter()
        .map(|page| MdBacklinksPage {
            path: page.path.clone(),
            exists: page.exists,
            backlinks: Vec::new(),
        })
        .collect::<Vec<_>>();

    for source in &corpus_files {
        match graph::analyze_edges(source, &root) {
            Ok(edges) => {
                for edge in edges {
                    if edge.target_type != TargetType::File {
                        continue;
                    }
                    let Some(resolved) = edge.resolved else {
                        continue;
                    };
                    let Some(page_idx) = focus_index.get(&normalized_path_key(&resolved)) else {
                        continue;
                    };
                    pages[*page_idx].backlinks.push(MdBacklink {
                        source: edge.source,
                        line_num: edge.line_num,
                        kind: edge.kind,
                        raw: edge.raw,
                    });
                }
            }
            Err(err) => warnings.push(format!("file_error {}: {err}", source.display_path)),
        }
    }

    let data = MdBacklinksData {
        focus_count: pages.len(),
        corpus_files: corpus_files.len(),
        total_backlinks: pages.iter().map(|page| page.backlinks.len()).sum(),
        pages,
    };

    print(data, warnings, &root, &from, !args.no_gitignore, ctx.print)?;
    Ok(0)
}

fn discover_corpus(
    from: &[String],
    root: &Path,
    respect_gitignore: bool,
) -> Result<(Vec<SourceFile>, Vec<String>)> {
    let mut files = BTreeMap::new();
    let mut warnings = Vec::new();

    for source in from {
        let path = rooted_path(source, root);
        if path.is_file() {
            if is_markdown_file(&path) {
                insert_source(&mut files, path, root);
            } else {
                warnings.push(format!("non-markdown corpus file skipped: {source}"));
            }
        } else if path.is_dir() {
            for file in walk_markdown_dir(&path, respect_gitignore)? {
                insert_source(&mut files, file, root);
            }
        } else if source.contains(GLOB_CHARS) {
            let pattern = rooted_path(source, root)
                .to_string_lossy()
                .replace('\\', "/");
            let mut matched = glob(&pattern)
                .with_context(|| format!("invalid glob pattern: {source}"))?
                .filter_map(Result::ok)
                .filter(|path| path.is_file() && is_markdown_file(path))
                .collect::<Vec<_>>();
            matched.sort();
            if matched.is_empty() {
                warnings.push(format!("corpus source not found: {source}"));
            } else {
                for file in matched {
                    insert_source(&mut files, file, root);
                }
            }
        } else {
            warnings.push(format!("corpus source not found: {source}"));
        }
    }

    Ok((files.into_values().collect(), warnings))
}

fn rooted_path(source: &str, root: &Path) -> PathBuf {
    let path = PathBuf::from(source);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn walk_markdown_dir(root: &Path, respect_gitignore: bool) -> Result<Vec<PathBuf>> {
    let mut walker = WalkBuilder::new(root);
    walker
        .hidden(false)
        .git_ignore(respect_gitignore)
        .git_global(respect_gitignore)
        .git_exclude(respect_gitignore)
        .sort_by_file_name(sort_entry_name);

    walker.filter_entry(move |entry| {
        if !respect_gitignore {
            return true;
        }
        let name = entry.file_name().to_str().unwrap_or("");
        !ALWAYS_SKIP.contains(&name)
    });

    let mut files = Vec::new();
    for entry in walker.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.is_file() && is_markdown_file(path) {
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}

fn insert_source(files: &mut BTreeMap<String, SourceFile>, path: PathBuf, root: &Path) {
    let display_path = display_path(&path, root);
    files.insert(display_path.clone(), SourceFile { path, display_path });
}

fn normalize_focus_page(input: &str, root: &Path) -> FocusPage {
    let target = strip_fragment_query(input).replace('\\', "/");
    let path = if let Some(stripped) = target.strip_prefix('/') {
        root.join(stripped)
    } else {
        let path = PathBuf::from(&target);
        if path.is_absolute() {
            path
        } else {
            root.join(path)
        }
    };

    FocusPage {
        path: display_path(&path, root),
        exists: path.exists(),
    }
}

fn dedupe_focus_pages(pages: impl IntoIterator<Item = FocusPage>) -> Vec<FocusPage> {
    let mut seen = std::collections::BTreeSet::new();
    pages
        .into_iter()
        .filter(|page| seen.insert(normalized_path_key(&page.path)))
        .collect()
}

fn normalized_path_key(path: &str) -> String {
    let mut parts = Vec::new();
    for component in Path::new(path).components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            _ => {}
        }
    }
    parts.join("/")
}

fn is_markdown_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    MARKDOWN_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

fn strip_fragment_query(target: &str) -> &str {
    target.split(['#', '?']).next().unwrap_or(target).trim()
}

fn sort_entry_name(a: &OsStr, b: &OsStr) -> std::cmp::Ordering {
    let a_s = a.to_string_lossy().to_lowercase();
    let b_s = b.to_string_lossy().to_lowercase();
    a_s.cmp(&b_s)
}

fn print(
    data: MdBacklinksData,
    warnings: Vec<String>,
    root: &Path,
    from: &[String],
    respect_gitignore: bool,
    mode: PrintMode,
) -> Result<()> {
    match mode {
        PrintMode::Json => {
            let payload = Envelope {
                ok: true,
                command: "md-backlinks",
                data,
                warnings,
                meta: serde_json::json!({
                    "cwd": root_display(root),
                    "from": from,
                    "respect_gitignore": respect_gitignore,
                    "builtin_skip": respect_gitignore,
                    "extensions": MARKDOWN_EXTENSIONS,
                }),
            };
            output::print_json(&payload)?;
        }
        _ => print_compact(&data, &warnings, respect_gitignore),
    }
    Ok(())
}

fn root_display(root: &Path) -> String {
    root.to_string_lossy().replace('\\', "/")
}

fn print_compact(data: &MdBacklinksData, warnings: &[String], respect_gitignore: bool) {
    println!(
        "# focus={} corpus_files={} backlinks={} gitignore={} builtin_skip={}",
        data.focus_count,
        data.corpus_files,
        data.total_backlinks,
        respect_gitignore,
        respect_gitignore
    );

    for warning in warnings {
        println!("! {warning}");
    }

    for page in &data.pages {
        println!(
            "@ {} exists={} backlinks={}",
            page.path,
            page.exists,
            page.backlinks.len()
        );
        for backlink in &page.backlinks {
            println!(
                "{}:L{}|{}|{}",
                backlink.source,
                backlink.line_num,
                kind_name(&backlink.kind),
                json_string(&backlink.raw)
            );
        }
    }
}

fn kind_name(kind: &LinkKind) -> &'static str {
    match kind {
        LinkKind::Markdown => "markdown",
        LinkKind::Image => "image",
        LinkKind::Wiki => "wiki",
        LinkKind::CodeSpan => "code_span",
        LinkKind::Angle => "angle",
        LinkKind::SiyuanBlock => "siyuan_block",
    }
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization cannot fail")
}
