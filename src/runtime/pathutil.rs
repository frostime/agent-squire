//! Path display helpers shared across builtins.
//!
//! All display helpers normalize Windows backslashes to forward slashes so
//! agent-facing output is stable across platforms.

use std::path::Path;

/// Render `path` relative to `base`, falling back to the raw path when it is
/// not under `base`. Backslashes are normalized to forward slashes.
///
/// Equivalent to the inline helper previously duplicated in `info` and
/// `read-range`: `strip_prefix(base).unwrap_or(path)` then slash-normalize.
pub fn display_relative(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Render `path` relative to `base` with a canonicalize-based fallback.
///
/// If `path` is not under `base` by literal prefix, both sides are
/// canonicalized and re-stripped; if still not under `base`, the canonicalized
/// (or raw) absolute path is returned. Always slash-normalizes.
///
/// Replaces the inline `display_path` previously shared verbatim by
/// `md_links::sources` and (via that import) `md_backlinks`, so paths reachable
/// through relative inputs or symlinks still display relative to the workspace.
pub fn display_relative_fallback(path: &Path, base: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(base) {
        return relative.to_string_lossy().replace('\\', "/");
    }
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let base_abs = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    abs.strip_prefix(&base_abs)
        .unwrap_or(&abs)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Slash-normalize a path's display form without re-basing it.
///
/// `path.to_string_lossy()` with backslashes rewritten to forward slashes.
/// Used by builtins that only need a stable display string and do not rebase
/// against a working directory (e.g. zip entries, template labels, root paths).
pub fn slash_normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Slash-normalize an already-owned string in place of a `Path`.
pub fn slash_normalize_str(s: &str) -> String {
    s.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn display_relative_strips_base_and_normalizes_slashes() {
        let base = PathBuf::from("/repo");
        let path = PathBuf::from("/repo/src/main.rs");
        assert_eq!(display_relative(&path, &base), "src/main.rs");
    }

    #[test]
    fn display_relative_falls_back_to_raw_when_not_under_base() {
        let base = PathBuf::from("/repo");
        let path = PathBuf::from("/other/x.rs");
        assert_eq!(display_relative(&path, &base), "/other/x.rs");
    }

    #[test]
    fn display_relative_fallback_strips_after_canonicalize() {
        // path produced as a relative child of base (simulating a walker-built
        // path) should strip to a relative display even without literal prefix.
        let dir = std::env::current_dir().unwrap();
        let base = dir.join("tmp_pf");
        std::fs::create_dir_all(&base).unwrap();
        let child = base.join("a.md");
        std::fs::write(&child, "x").unwrap();
        // PathBuf built relative to cwd (mirrors walker for toc/md_links):
        let rel = PathBuf::from("tmp_pf/a.md");
        assert_eq!(display_relative_fallback(&rel, &dir), "tmp_pf/a.md");
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn slash_normalize_rewrites_backslashes() {
        let path = PathBuf::from(r"src\nested\a.rs");
        assert_eq!(slash_normalize(&path), "src/nested/a.rs");
    }

    #[test]
    fn slash_normalize_str_rewrites_backslashes() {
        assert_eq!(slash_normalize_str(r"a\b\c"), "a/b/c");
    }
}
