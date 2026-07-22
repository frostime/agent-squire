//! Source resolution: expand file/dir/glob inputs into a typed list.
//!
//! `toc`, `md-links`, `md-backlinks`, and `file-info` all accept a list of
//! inputs that may be files, directories, or glob patterns and produce a
//! filtered file list plus a list of inputs that could not be resolved
//! ("unresolved"). This module owns that loop; each builtin declares its
//! policy structurally and injects a `map` closure to convert a resolved
//! `PathBuf` into its own output item.
//!
//! See `.sspec/spec-docs/builtin-source-resolver.md` for the policy axis
//! table, the caller mapping, and the equivalence guarantees.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use glob::glob;
use ignore::WalkBuilder;
use walkdir::WalkDir;

/// Directories skipped when `IgnoreSources::BUILTIN_DIRS` is active. Shared
/// with `tree` and `gather`; defined here so all three reference one source.
pub const ALWAYS_SKIP: &[&str] = &[
    ".git",
    "__pycache__",
    "node_modules",
    ".pytest_cache",
    ".mypy_cache",
];

/// Characters that mark a string as a glob pattern.
pub const GLOB_CHARS: &[char] = &['*', '?', '['];

/// Markdown file extensions (case-insensitive).
pub const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown"];

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct IgnoreSources: u8 {
        /// Rules from `.ignore` files.
        const DOT_IGNORE = 1 << 0;
        /// Rules from `.gitignore`, global Git ignore, and Git exclude files.
        const GIT = 1 << 1;
        /// Directory names listed in [`ALWAYS_SKIP`].
        const BUILTIN_DIRS = 1 << 2;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectorySelection {
    /// Include every file accepted by the caller's predicate.
    All,
    /// Exclude files according to the selected ignore-rule sources.
    Filtered(IgnoreSources),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobDirectoryMode {
    /// Ignore directories returned by a glob match.
    Skip,
    /// Recursively expand directories returned by a glob match.
    Recurse,
}

/// How duplicate paths are collapsed across inputs.
pub enum Dedup<'a> {
    /// Preserve every accepted occurrence.
    None,
    /// Collapse by `path.canonicalize()`.
    Canonicalize,
    /// Collapse by a caller-supplied key (typically the display path). The
    /// last mapped item wins; final output is sorted by key.
    ByKey(&'a dyn Fn(&Path, &Path) -> String),
}

pub struct SourcePolicy<'a, T> {
    /// Workspace root forwarded to `map` and the `Dedup::ByKey` key. Should
    /// equal the `CommandContext` cwd.
    pub root: &'a Path,
    pub directory_selection: DirectorySelection,
    pub glob_directory_mode: GlobDirectoryMode,
    /// File acceptance test applied to files found via directory recursion or
    /// glob expansion. Inject `is_markdown_file` for Markdown builtins,
    /// `|_| true` for `file-info`, or any custom predicate.
    pub accept_file: &'a dyn Fn(&Path) -> bool,
    /// When true, an input naming an existing file must still pass
    /// `accept_file`; rejection becomes an unresolved entry. `md-backlinks`
    /// sets this true; `toc`/`md-links`/`file-info` leave it false.
    pub filter_explicit_file: bool,
    /// When true, glob matches must also pass `accept_file`; when false, every
    /// glob-matched file is accepted (matching the legacy `toc`/`md-links`/
    /// `file-info` glob branch, which only filtered by `is_file()`).
    /// `md-backlinks` sets this true so non-markdown glob matches are skipped.
    pub filter_glob: bool,
    pub dedup: Dedup<'a>,
    pub max_files: Option<usize>,
    /// Convert an accepted `PathBuf` into the caller's output item. Returning
    /// `None` drops the path (e.g. when further validation fails).
    pub map: &'a dyn Fn(PathBuf, &Path) -> Option<T>,
}

/// Resolve `inputs` into `(Vec<T>, Vec<String>)`. The second element is the
/// list of inputs that could not be resolved (missing path, empty glob match,
/// or explicit-file rejection when `filter_explicit_file` is set). An empty
/// `inputs` list is treated as `["."]`, matching the legacy per-builtin
/// default.
pub fn resolve<T>(inputs: &[String], policy: SourcePolicy<'_, T>) -> Result<(Vec<T>, Vec<String>)> {
    let mut out: Vec<T> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    let mut seen_canon: BTreeSet<PathBuf> = BTreeSet::new();
    let mut keyed: BTreeMap<String, T> = BTreeMap::new();

    let work_list: Vec<String> = if inputs.is_empty() {
        vec![".".to_string()]
    } else {
        inputs.to_vec()
    };

    for source in &work_list {
        if max_reached(policy.max_files, resolved_len(&out, &keyed, &policy.dedup)) {
            break;
        }

        let path = PathBuf::from(source);
        if path.is_dir() {
            expand_directory_into(&path, &policy, &mut seen_canon, &mut keyed, &mut out)?;
        } else if path.is_file() {
            let accept = !policy.filter_explicit_file || (policy.accept_file)(&path);
            if accept {
                accept_into(
                    &path,
                    policy.root,
                    &policy,
                    &mut seen_canon,
                    &mut keyed,
                    &mut out,
                );
            } else {
                unresolved.push(source.clone());
            }
        } else if has_glob_magic(source) {
            let mut any = false;
            for entry in glob(source).with_context(|| format!("invalid glob pattern: {source}"))? {
                if max_reached(policy.max_files, resolved_len(&out, &keyed, &policy.dedup)) {
                    break;
                }
                let matched = match entry {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                if matched.is_dir() {
                    if policy.glob_directory_mode == GlobDirectoryMode::Recurse {
                        any = true;
                        expand_directory_into(
                            &matched,
                            &policy,
                            &mut seen_canon,
                            &mut keyed,
                            &mut out,
                        )?;
                    }
                    continue;
                }
                if !matched.is_file() || (policy.filter_glob && !(policy.accept_file)(&matched)) {
                    continue;
                }
                any = true;
                accept_into(
                    &matched,
                    policy.root,
                    &policy,
                    &mut seen_canon,
                    &mut keyed,
                    &mut out,
                );
            }
            if !any {
                unresolved.push(source.clone());
            }
        } else {
            unresolved.push(source.clone());
        }
    }

    if matches!(&policy.dedup, Dedup::ByKey(_)) {
        out = keyed.into_values().collect();
    }
    Ok((out, unresolved))
}

fn expand_directory_into<T>(
    path: &Path,
    policy: &SourcePolicy<'_, T>,
    seen_canon: &mut BTreeSet<PathBuf>,
    keyed: &mut BTreeMap<String, T>,
    out: &mut Vec<T>,
) -> Result<()> {
    if policy.max_files.is_some() {
        // `file-info --max-files` must not materialize an entire large
        // directory before applying its cap.
        for_each_dir_file(
            path,
            policy.directory_selection,
            policy.accept_file,
            |file| {
                accept_into(file.as_path(), policy.root, policy, seen_canon, keyed, out);
                !max_reached(policy.max_files, resolved_len(out, keyed, &policy.dedup))
            },
        )?;
    } else {
        for file in walk_dir(path, policy.directory_selection, policy.accept_file)? {
            accept_into(&file, policy.root, policy, seen_canon, keyed, out);
        }
    }
    Ok(())
}

fn accept_into<T>(
    path: &Path,
    root: &Path,
    policy: &SourcePolicy<'_, T>,
    seen_canon: &mut BTreeSet<PathBuf>,
    keyed: &mut BTreeMap<String, T>,
    out: &mut Vec<T>,
) {
    match &policy.dedup {
        Dedup::None => {
            if let Some(t) = (policy.map)(path.to_path_buf(), root) {
                out.push(t);
            }
        }
        Dedup::Canonicalize => {
            let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            if !seen_canon.insert(canon) {
                return;
            }
            if let Some(t) = (policy.map)(path.to_path_buf(), root) {
                out.push(t);
            }
        }
        Dedup::ByKey(key) => {
            if let Some(t) = (policy.map)(path.to_path_buf(), root) {
                keyed.insert(key(path, root), t);
            }
        }
    }
}

fn resolved_len<T>(out: &[T], keyed: &BTreeMap<String, T>, dedup: &Dedup<'_>) -> usize {
    if matches!(dedup, Dedup::ByKey(_)) {
        keyed.len()
    } else {
        out.len()
    }
}

fn max_reached(max_files: Option<usize>, len: usize) -> bool {
    matches!(max_files, Some(n) if len >= n)
}

fn walk_dir(
    root: &Path,
    selection: DirectorySelection,
    accept: &dyn Fn(&Path) -> bool,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for_each_dir_file(root, selection, accept, |path| {
        files.push(path);
        true
    })?;
    files.sort();
    Ok(files)
}

fn for_each_dir_file(
    root: &Path,
    selection: DirectorySelection,
    accept: &dyn Fn(&Path) -> bool,
    mut visit: impl FnMut(PathBuf) -> bool,
) -> Result<()> {
    match selection {
        DirectorySelection::All => {
            for entry in WalkDir::new(root).sort_by_file_name() {
                let Ok(entry) = entry else { continue };
                let path = entry.into_path();
                if path.is_file() && accept(&path) && !visit(path) {
                    break;
                }
            }
        }
        DirectorySelection::Filtered(sources) => {
            let git = sources.contains(IgnoreSources::GIT);
            let mut walker = WalkBuilder::new(root);
            walker
                .hidden(false)
                .ignore(sources.contains(IgnoreSources::DOT_IGNORE))
                .git_ignore(git)
                .git_global(git)
                .git_exclude(git)
                .sort_by_file_name(sort_entry_name);

            if sources.contains(IgnoreSources::BUILTIN_DIRS) {
                walker.filter_entry(|entry| {
                    let name = entry.file_name().to_str().unwrap_or("");
                    !ALWAYS_SKIP.contains(&name)
                });
            }

            for entry in walker.build() {
                let Ok(entry) = entry else { continue };
                let path = entry.path().to_path_buf();
                if path.is_file() && accept(&path) && !visit(path) {
                    break;
                }
            }
        }
    }
    Ok(())
}

pub fn has_glob_magic(s: &str) -> bool {
    s.chars().any(|c| GLOB_CHARS.contains(&c))
}

pub fn is_markdown_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    MARKDOWN_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

pub fn sort_entry_name(a: &OsStr, b: &OsStr) -> std::cmp::Ordering {
    let a_s = a.to_string_lossy().to_lowercase();
    let b_s = b.to_string_lossy().to_lowercase();
    a_s.cmp(&b_s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;

    fn write(folder: &Path, rel: &str, body: &str) -> PathBuf {
        let p = folder.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, body).unwrap();
        p
    }

    /// Render each path relative to `root` via pathutil so the assertions are
    /// independent of how the walker prefixes paths (abs vs ./). This mirrors
    /// what real callers do inside their `map`.
    fn display(paths: &[PathBuf], root: &Path) -> Vec<String> {
        let mut v: Vec<String> = paths
            .iter()
            .map(|p| crate::shared::path::display_relative_fallback(p, root))
            .collect();
        v.sort();
        v
    }

    /// Absolute-input variant of `SourcePolicy` map for tests that want raw
    /// PathBuf (counting dedup without display translation).
    fn ident_map() -> &'static dyn Fn(PathBuf, &Path) -> Option<PathBuf> {
        &|p, _| Some(p)
    }

    #[test]
    fn all_selection_ignores_every_exclusion_source() {
        let dir = tempdir().unwrap();
        write(dir.path(), "a.md", "");
        write(dir.path(), ".hidden/b.md", "");
        write(dir.path(), "git-ignored.md", "");
        write(dir.path(), "dot-ignored.md", "");
        write(dir.path(), "node_modules/x.md", "");
        fs::write(dir.path().join(".gitignore"), "git-ignored.md\n").unwrap();
        fs::write(dir.path().join(".ignore"), "dot-ignored.md\n").unwrap();

        let input = dir.path().to_string_lossy().to_string();
        let (files, unresolved) = resolve(
            &[input],
            SourcePolicy {
                root: dir.path(),
                directory_selection: DirectorySelection::All,
                glob_directory_mode: GlobDirectoryMode::Skip,
                accept_file: &is_markdown_file,
                filter_explicit_file: false,
                filter_glob: true,
                dedup: Dedup::None,
                max_files: None,
                map: ident_map(),
            },
        )
        .unwrap();

        assert_eq!(
            display(&files, dir.path()),
            vec![
                ".hidden/b.md",
                "a.md",
                "dot-ignored.md",
                "git-ignored.md",
                "node_modules/x.md",
            ]
        );
        assert!(unresolved.is_empty());
    }

    #[test]
    fn filtered_selection_applies_selected_ignore_sources() {
        let dir = tempdir().unwrap();
        write(dir.path(), "a.md", "");
        write(dir.path(), "git-ignored.md", "");
        write(dir.path(), "dot-ignored.md", "");
        write(dir.path(), "node_modules/x.md", "");
        fs::write(dir.path().join(".gitignore"), "git-ignored.md\n").unwrap();
        fs::write(dir.path().join(".ignore"), "dot-ignored.md\n").unwrap();

        let input = dir.path().to_string_lossy().to_string();
        let (files, _) = resolve(
            &[input],
            SourcePolicy {
                root: dir.path(),
                directory_selection: DirectorySelection::Filtered(
                    IgnoreSources::DOT_IGNORE | IgnoreSources::BUILTIN_DIRS,
                ),
                glob_directory_mode: GlobDirectoryMode::Skip,
                accept_file: &is_markdown_file,
                filter_explicit_file: false,
                filter_glob: true,
                dedup: Dedup::None,
                max_files: None,
                map: ident_map(),
            },
        )
        .unwrap();

        assert_eq!(display(&files, dir.path()), vec!["a.md", "git-ignored.md"]);
    }

    #[test]
    fn explicit_non_markdown_accepted_unless_filter_explicit_file() {
        let dir = tempdir().unwrap();
        write(dir.path(), "note.txt", "");
        let input = dir.path().join("note.txt").to_string_lossy().to_string();

        // filter_explicit_file=false: any named file accepted (toc/md-links/file-info).
        let (files, unresolved) = resolve(
            std::slice::from_ref(&input),
            SourcePolicy {
                root: dir.path(),
                directory_selection: DirectorySelection::All,
                glob_directory_mode: GlobDirectoryMode::Skip,
                accept_file: &is_markdown_file,
                filter_explicit_file: false,
                filter_glob: true,
                dedup: Dedup::None,
                max_files: None,
                map: ident_map(),
            },
        )
        .unwrap();
        assert_eq!(files.len(), 1);
        assert!(unresolved.is_empty());

        // filter_explicit_file=true: non-markdown explicit rejected (md-backlinks).
        let (files, unresolved) = resolve(
            &[input],
            SourcePolicy {
                root: dir.path(),
                directory_selection: DirectorySelection::All,
                glob_directory_mode: GlobDirectoryMode::Skip,
                accept_file: &is_markdown_file,
                filter_explicit_file: true,
                filter_glob: true,
                dedup: Dedup::None,
                max_files: None,
                map: ident_map(),
            },
        )
        .unwrap();
        assert!(files.is_empty());
        assert_eq!(unresolved.len(), 1);
    }

    #[test]
    fn glob_missing_and_directory_filter() {
        let dir = tempdir().unwrap();
        write(dir.path(), "src/a.md", "");
        write(dir.path(), "src/b.txt", "");
        let hit = dir.path().join("src/**/*.md").to_string_lossy().to_string();
        let miss = dir.path().join("none/*.md").to_string_lossy().to_string();
        let (files, unresolved) = resolve(
            &[hit, miss],
            SourcePolicy {
                root: dir.path(),
                directory_selection: DirectorySelection::All,
                glob_directory_mode: GlobDirectoryMode::Skip,
                accept_file: &is_markdown_file,
                filter_explicit_file: false,
                filter_glob: true,
                dedup: Dedup::None,
                max_files: None,
                map: ident_map(),
            },
        )
        .unwrap();

        assert_eq!(display(&files, dir.path()), vec!["src/a.md".to_string()]);
        assert_eq!(unresolved.len(), 1);
    }

    #[test]
    fn glob_directory_mode_controls_recursive_expansion() {
        let dir = tempdir().unwrap();
        write(dir.path(), "packages/alpha/nested.md", "");
        let pattern = dir
            .path()
            .join("packages/*")
            .to_string_lossy()
            .replace('\\', "/");

        let resolve_with = |glob_directory_mode| {
            resolve(
                std::slice::from_ref(&pattern),
                SourcePolicy {
                    root: dir.path(),
                    directory_selection: DirectorySelection::All,
                    glob_directory_mode,
                    accept_file: &is_markdown_file,
                    filter_explicit_file: false,
                    filter_glob: true,
                    dedup: Dedup::None,
                    max_files: None,
                    map: ident_map(),
                },
            )
            .unwrap()
        };

        let (skipped, unresolved) = resolve_with(GlobDirectoryMode::Skip);
        assert!(skipped.is_empty());
        assert_eq!(unresolved, vec![pattern.clone()]);

        let (recursed, unresolved) = resolve_with(GlobDirectoryMode::Recurse);
        assert_eq!(
            display(&recursed, dir.path()),
            vec!["packages/alpha/nested.md"]
        );
        assert!(unresolved.is_empty());
    }

    #[test]
    fn recursed_empty_glob_directory_counts_as_resolved() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("packages/empty")).unwrap();
        let pattern = dir
            .path()
            .join("packages/*")
            .to_string_lossy()
            .replace('\\', "/");

        let (files, unresolved) = resolve(
            &[pattern],
            SourcePolicy {
                root: dir.path(),
                directory_selection: DirectorySelection::All,
                glob_directory_mode: GlobDirectoryMode::Recurse,
                accept_file: &is_markdown_file,
                filter_explicit_file: false,
                filter_glob: true,
                dedup: Dedup::None,
                max_files: None,
                map: ident_map(),
            },
        )
        .unwrap();

        assert!(files.is_empty());
        assert!(unresolved.is_empty());
    }

    #[test]
    fn canonicalize_dedup_collapses_overlapping_inputs() {
        let dir = tempdir().unwrap();
        write(dir.path(), "a.md", "");
        let file_input = dir.path().join("a.md").to_string_lossy().to_string();
        let dir_input = dir.path().to_string_lossy().to_string();
        let (files, _) = resolve(
            &[file_input, dir_input],
            SourcePolicy {
                root: dir.path(),
                directory_selection: DirectorySelection::All,
                glob_directory_mode: GlobDirectoryMode::Skip,
                accept_file: &is_markdown_file,
                filter_explicit_file: false,
                filter_glob: true,
                dedup: Dedup::Canonicalize,
                max_files: None,
                map: ident_map(),
            },
        )
        .unwrap();

        // Both the explicit file and the dir walk find the same file; canonicalize
        // dedup must emit it once.
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn max_files_stops_directory_scan_after_accepting_limit() {
        let dir = tempdir().unwrap();
        write(dir.path(), "a.md", "");
        write(dir.path(), "b.md", "");
        write(dir.path(), "c.md", "");
        let input = dir.path().to_string_lossy().to_string();
        let visited = AtomicUsize::new(0);
        let accept = |_: &Path| {
            visited.fetch_add(1, Ordering::SeqCst);
            true
        };
        let (files, _) = resolve(
            &[input],
            SourcePolicy {
                root: dir.path(),
                directory_selection: DirectorySelection::All,
                glob_directory_mode: GlobDirectoryMode::Skip,
                accept_file: &accept,
                filter_explicit_file: false,
                filter_glob: true,
                dedup: Dedup::None,
                max_files: Some(2),
                map: ident_map(),
            },
        )
        .unwrap();

        assert_eq!(files.len(), 2);
        assert_eq!(visited.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn bykey_dedup_keeps_last_item_and_sorts_by_key() {
        let dir = tempdir().unwrap();
        write(dir.path(), "a.md", "");
        write(dir.path(), "z.md", "");
        fs::create_dir_all(dir.path().join("alias")).unwrap();

        let z = dir.path().join("z.md").to_string_lossy().to_string();
        let first_a = dir.path().join("a.md").to_string_lossy().to_string();
        let last_a = dir
            .path()
            .join("alias")
            .join("..")
            .join("a.md")
            .to_string_lossy()
            .to_string();
        let key = |p: &Path, _: &Path| p.file_name().unwrap().to_string_lossy().to_string();
        let (files, _) = resolve(
            &[z.clone(), first_a, last_a.clone()],
            SourcePolicy {
                root: dir.path(),
                directory_selection: DirectorySelection::All,
                glob_directory_mode: GlobDirectoryMode::Skip,
                accept_file: &is_markdown_file,
                filter_explicit_file: false,
                filter_glob: true,
                dedup: Dedup::ByKey(&key),
                max_files: None,
                map: ident_map(),
            },
        )
        .unwrap();

        assert_eq!(files, vec![PathBuf::from(last_a), PathBuf::from(z)]);
    }

    #[test]
    fn is_markdown_file_matches_md_and_markdown_case_insensitive() {
        assert!(is_markdown_file(Path::new("a.MD")));
        assert!(is_markdown_file(Path::new("b.markdown")));
        assert!(!is_markdown_file(Path::new("c.txt")));
        assert!(!is_markdown_file(Path::new("noext")));
    }

    #[test]
    fn has_glob_magic_detects_pattern_chars() {
        assert!(has_glob_magic("a/*.md"));
        assert!(has_glob_magic("a?.md"));
        assert!(has_glob_magic("a[0-9].md"));
        assert!(!has_glob_magic("normal/path.md"));
    }
}
