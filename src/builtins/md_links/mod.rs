pub(crate) mod graph;
pub(crate) mod model;
mod output;
pub(crate) mod parse;
pub(crate) mod resolve;
pub(crate) mod sources;

use anyhow::Result;
use clap::Args;

use crate::cli::CommandContext;

use self::model::{MdLinksData, MdLinksFile, TargetType};

#[derive(Args, Debug)]
#[command(
    long_about = r#"Extract Markdown link-like references and resolve file targets against the effective current working directory.

Inputs may be Markdown files, directories, or glob patterns. Directories are searched recursively for .md files. JSON output is graph-ready; compact output groups dense records by source file."#,
    after_help = r#"Examples:
    asq md-links README.md
    asq --cwd . md-links docs
    asq --print json md-links docs/"#
)]
pub struct MdLinksArgs {
    #[arg(
        default_value = ".",
        help = "Markdown files, directories, or glob patterns"
    )]
    pub sources: Vec<String>,
}

pub fn run(args: MdLinksArgs, ctx: &CommandContext) -> Result<u8> {
    let root = ctx.cwd.clone();
    let sources = if args.sources.is_empty() {
        vec![".".to_string()]
    } else {
        args.sources
    };
    let (source_files, missing) = sources::resolve_sources(&sources, &root)?;

    if source_files.is_empty() && !missing.is_empty() {
        anyhow::bail!("No Markdown files found for: {}", missing.join(", "));
    }

    let files = source_files
        .iter()
        .map(|source| analyze_file(source, &root))
        .collect::<Vec<_>>();

    let data = MdLinksData {
        count: files.len(),
        total_links: files.iter().map(|file| file.links.len()).sum(),
        total_file_links: files
            .iter()
            .flat_map(|file| &file.links)
            .filter(|link| link.target_type == TargetType::File)
            .count(),
        total_existing_file_links: files
            .iter()
            .flat_map(|file| &file.links)
            .filter(|link| link.target_type == TargetType::File && link.exists == Some(true))
            .count(),
        files,
    };

    let warnings = missing
        .iter()
        .map(|source| format!("source not found: {source}"))
        .collect();
    output::print(data, warnings, root_display(&root), ctx.print)?;
    Ok(0)
}

fn root_display(root: &std::path::Path) -> String {
    root.to_string_lossy().replace('\\', "/")
}

fn analyze_file(source: &model::SourceFile, root: &std::path::Path) -> MdLinksFile {
    let content = match std::fs::read_to_string(&source.path) {
        Ok(content) => content,
        Err(err) => {
            return MdLinksFile {
                path: source.display_path.clone(),
                links: vec![],
                error: Some(err.to_string()),
            };
        }
    };

    let links = parse::parse_links(&content)
        .into_iter()
        .filter_map(|raw| resolve::resolve_link(raw, &source.path, root))
        .collect();

    MdLinksFile {
        path: source.display_path.clone(),
        links,
        error: None,
    }
}
