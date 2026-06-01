use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use glob::glob;

use super::model::SourceFile;

const GLOB_CHARS: &[char] = &['*', '?', '['];

pub fn resolve_sources(
    sources: &[String],
    workspace: &Path,
) -> Result<(Vec<SourceFile>, Vec<String>)> {
    let mut files = Vec::new();
    let mut missing = Vec::new();

    for source in sources {
        let path = PathBuf::from(source);
        if path.is_dir() {
            let mut found = walkdir::WalkDir::new(&path)
                .sort_by_file_name()
                .into_iter()
                .filter_map(Result::ok)
                .map(|entry| entry.into_path())
                .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
                .collect::<Vec<_>>();
            found.sort();
            files.extend(found.into_iter().map(|path| source_file(path, workspace)));
        } else if path.is_file() {
            files.push(source_file(path, workspace));
        } else if source.contains(GLOB_CHARS) {
            let mut matched: Vec<_> = glob(source)
                .with_context(|| format!("invalid glob pattern: {source}"))?
                .filter_map(Result::ok)
                .filter(|path| path.is_file())
                .collect();
            matched.sort();
            if matched.is_empty() {
                missing.push(source.clone());
            } else {
                files.extend(matched.into_iter().map(|path| source_file(path, workspace)));
            }
        } else {
            missing.push(source.clone());
        }
    }

    Ok((files, missing))
}

fn source_file(path: PathBuf, workspace: &Path) -> SourceFile {
    let display_path = display_path(&path, workspace);
    SourceFile { path, display_path }
}

pub fn display_path(path: &Path, workspace: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(workspace) {
        return relative.to_string_lossy().replace('\\', "/");
    }

    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let workspace_abs = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    abs.strip_prefix(&workspace_abs)
        .unwrap_or(&abs)
        .to_string_lossy()
        .replace('\\', "/")
}
