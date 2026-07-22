use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use glob::glob;
use ignore::WalkBuilder;

use crate::shared::file_sources::ALWAYS_SKIP;

pub fn expand_dir(cwd: &Path, path: &Path, respect_gitignore: bool) -> Result<Vec<PathBuf>> {
    let root = resolve_path(cwd, path);
    if !root.is_dir() {
        bail!("Directory not found: {}", path.display());
    }

    let mut walker = WalkBuilder::new(&root);
    configure_walker(&mut walker, cwd, respect_gitignore);
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
        if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            files.push(normalize_path(relative_to(cwd, entry.path())));
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

pub fn expand_glob(cwd: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    let effective = if Path::new(pattern).is_absolute() {
        pattern.to_string()
    } else {
        cwd.join(pattern).display().to_string()
    };

    let mut files = Vec::new();
    for entry in glob(&effective).with_context(|| format!("invalid glob pattern: {pattern}"))? {
        let path = match entry {
            Ok(p) => p,
            Err(_) => continue,
        };
        if path.is_file() {
            files.push(normalize_path(relative_to(cwd, &path)));
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

pub fn render_tree(cwd: &Path, path: &Path, respect_gitignore: bool) -> Result<String> {
    let root = resolve_path(cwd, path);
    if !root.is_dir() {
        bail!("Directory not found: {}", path.display());
    }

    let files = expand_dir(cwd, path, respect_gitignore)?;
    let root_label = path.display().to_string();
    let mut out = String::new();
    out.push_str(&root_label);
    if !root_label.ends_with('/') && !root_label.ends_with('\\') {
        out.push('/');
    }
    out.push('\n');

    for file in files {
        out.push_str("  ");
        out.push_str(&file.display().to_string());
        out.push('\n');
    }
    Ok(out)
}

pub fn fzf_files(cwd: &Path, respect_gitignore: bool) -> Result<Vec<PathBuf>> {
    expand_dir(cwd, Path::new("."), respect_gitignore)
}

pub fn fzf_dirs(cwd: &Path, respect_gitignore: bool) -> Result<Vec<PathBuf>> {
    let mut walker = WalkBuilder::new(cwd);
    configure_walker(&mut walker, cwd, respect_gitignore);
    walker.filter_entry(move |entry| {
        if !respect_gitignore {
            return true;
        }
        let name = entry.file_name().to_str().unwrap_or("");
        !ALWAYS_SKIP.contains(&name)
    });

    let mut dirs = Vec::new();
    for entry in walker.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.path() == cwd {
            continue;
        }
        if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir())
        {
            dirs.push(normalize_path(relative_to(cwd, entry.path())));
        }
    }
    dirs.sort();
    dirs.dedup();
    Ok(dirs)
}

// SPEC: Default gather discovery respects the workspace .gitignore at cwd even
// when walking a subdirectory or a non-Git temp workspace. --no-gitignore is the
// explicit opt-out for dir expansion and interactive candidates.
fn configure_walker(walker: &mut WalkBuilder, cwd: &Path, respect_gitignore: bool) {
    walker
        .hidden(false)
        .git_ignore(respect_gitignore)
        .git_global(respect_gitignore)
        .git_exclude(respect_gitignore)
        .sort_by_file_name(|a, b| a.cmp(b));

    if respect_gitignore {
        walker.current_dir(cwd);
        let gitignore = cwd.join(".gitignore");
        if gitignore.is_file() {
            walker.add_ignore(gitignore);
        }
    }
}

fn resolve_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn relative_to(cwd: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(cwd).unwrap_or(path).to_path_buf()
}

fn normalize_path(path: PathBuf) -> PathBuf {
    PathBuf::from(path.display().to_string().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn expands_dir_sorted_and_ignores_gitignore() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "a").unwrap();
        fs::write(dir.path().join("src/b.rs"), "b").unwrap();
        fs::write(dir.path().join(".gitignore"), "src/b.rs\n").unwrap();

        let files = expand_dir(dir.path(), Path::new("src"), true).unwrap();
        assert_eq!(files, vec![PathBuf::from("src/a.rs")]);
    }

    #[test]
    fn no_gitignore_includes_ignored_files() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "a").unwrap();
        fs::write(dir.path().join("src/b.rs"), "b").unwrap();
        fs::write(dir.path().join(".gitignore"), "src/b.rs\n").unwrap();

        let files = expand_dir(dir.path(), Path::new("src"), false).unwrap();
        assert_eq!(
            files,
            vec![PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")]
        );
    }

    #[test]
    fn expands_glob_sorted() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/b.rs"), "b").unwrap();
        fs::write(dir.path().join("src/a.rs"), "a").unwrap();

        let files = expand_glob(dir.path(), "src/*.rs").unwrap();
        assert_eq!(
            files,
            vec![PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")]
        );
    }
}
