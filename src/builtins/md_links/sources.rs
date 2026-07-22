use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::builtins::source::{self, Dedup, GitignoreMode, SourcePolicy};
use crate::runtime::pathutil;

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
            gitignore: GitignoreMode::Off,
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
/// `pathutil::display_relative_fallback`.
pub fn display_path(path: &Path, workspace: &Path) -> String {
    pathutil::display_relative_fallback(path, workspace)
}
