use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::shared::file_sources::{
    self as source, Dedup, DirectorySelection, GlobDirectoryMode, SourcePolicy,
};
use crate::shared::path;

use super::model::SourceFile;

pub fn resolve_sources(
    sources: &[String],
    workspace: &Path,
) -> Result<(Vec<SourceFile>, Vec<String>)> {
    let md_only = |p: &Path| p.extension().is_some_and(|ext| ext == "md");
    source::resolve(
        sources,
        SourcePolicy {
            root: workspace,
            directory_selection: DirectorySelection::All,
            glob_directory_mode: GlobDirectoryMode::Skip,
            accept_file: &md_only,
            filter_explicit_file: false,
            filter_glob: false,
            dedup: Dedup::None,
            max_files: None,
            map: &|p, root| Some(source_file(p, root)),
        },
    )
}

fn source_file(path: PathBuf, root: &Path) -> SourceFile {
    let display_path = display_path(&path, root);
    SourceFile { path, display_path }
}

/// Display a resolved path relative to the workspace root, with a
/// canonicalize-based fallback so paths reached via relative inputs or
/// symlinks still display relative to the workspace. Delegates to
/// `path::display_relative_fallback`.
pub fn display_path(path: &Path, workspace: &Path) -> String {
    path::display_relative_fallback(path, workspace)
}
