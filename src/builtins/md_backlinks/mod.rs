use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Args;
use serde::Serialize;

use crate::builtins::md_links::graph;
use crate::builtins::md_links::model::{LinkKind, SourceFile, TargetType};
use crate::builtins::md_links::sources::display_path;
use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};
use crate::shared::file_sources::{
    self as source, Dedup, GitignoreMode, MARKDOWN_EXTENSIONS, SourcePolicy,
};

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
    // Corpus is keyed by display path so two inputs resolving to the same path
    // (e.g. an explicit file plus a dir walk) collapse to one SourceFile.
    let (files, unresolved) = source::resolve(
        from,
        SourcePolicy {
            root,
            gitignore: if respect_gitignore {
                GitignoreMode::Respect
            } else {
                GitignoreMode::Off
            },
            accept_file: &source::is_markdown_file,
            filter_explicit_file: true,
            filter_glob: true,
            dedup: Dedup::ByKey(&display_path_key),
            max_files: None,
            map: &|p, root| {
                let display = display_path(&p, root);
                Some(SourceFile {
                    path: p,
                    display_path: display,
                })
            },
        },
    )?;

    // Consolidate the reject/not-found reasons into a single warning wording.
    // The previous implementation distinguished "non-markdown corpus file
    // skipped" from "corpus source not found"; both now surface identically.
    let warnings = unresolved
        .iter()
        .map(|source| format!("corpus source not found: {source}"))
        .collect();
    Ok((files, warnings))
}

fn display_path_key(path: &Path, root: &Path) -> String {
    display_path(path, root)
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

fn strip_fragment_query(target: &str) -> &str {
    target.split(['#', '?']).next().unwrap_or(target).trim()
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
            let payload = Envelope::new("md-backlinks", data)
                .with_warnings(warnings)
                .with_meta(serde_json::json!({
                    "cwd": root_display(root),
                    "from": from,
                    "respect_gitignore": respect_gitignore,
                    "builtin_skip": respect_gitignore,
                    "extensions": MARKDOWN_EXTENSIONS,
                }));
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
