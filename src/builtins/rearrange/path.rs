use std::path::{Component, Path, PathBuf};

use crate::builtins::rearrange::error::{ErrorCode, RearrangeError, Result};

#[derive(Debug, Clone)]
pub struct ResolvedPath {
    pub display: String,
    pub abs: PathBuf,
    pub key: String,
}

pub struct PathResolver {
    cwd: PathBuf,
}

impl PathResolver {
    pub fn new(cwd: &Path) -> Result<Self> {
        let cwd = cwd
            .canonicalize()
            .map_err(|e| err(ErrorCode::IoError, format!("failed to resolve cwd: {e}")))?;
        Ok(Self { cwd })
    }

    pub fn resolve(&self, raw: &str) -> Result<ResolvedPath> {
        let raw_path = Path::new(raw);
        let candidate = if raw_path.is_absolute() {
            raw_path.to_path_buf()
        } else {
            self.cwd.join(raw_path)
        };
        let lexical = normalize_path(&candidate);

        let abs = if lexical.exists() {
            if !lexical.is_file() {
                return Err(err(ErrorCode::NotAFile, format!("not a file: {raw}")));
            }
            lexical
                .canonicalize()
                .map_err(|e| err(ErrorCode::IoError, format!("failed to resolve {raw}: {e}")))?
        } else {
            resolve_missing_path(&lexical)?
        };

        if !abs.starts_with(&self.cwd) {
            return Err(err(
                ErrorCode::PathEscapesCwd,
                format!("path escapes cwd: {raw}"),
            ));
        }

        Ok(ResolvedPath {
            display: normalize_display(raw),
            key: path_key(&abs),
            abs,
        })
    }
}

pub fn reject_prefix_conflicts(paths: &[ResolvedPath]) -> Result<()> {
    for (idx, a) in paths.iter().enumerate() {
        for b in paths.iter().skip(idx + 1) {
            if a.abs == b.abs {
                continue;
            }
            if a.abs.starts_with(&b.abs) || b.abs.starts_with(&a.abs) {
                return Err(err(
                    ErrorCode::PathConflict,
                    format!("target paths conflict: {} and {}", a.display, b.display),
                ));
            }
        }
    }
    Ok(())
}

fn resolve_missing_path(path: &Path) -> Result<PathBuf> {
    let mut ancestor = path.to_path_buf();
    let mut suffix = PathBuf::new();

    while !ancestor.exists() {
        let Some(name) = ancestor.file_name().map(|s| s.to_os_string()) else {
            return Err(err(
                ErrorCode::PathEscapesCwd,
                format!("path has no existing ancestor: {}", path.display()),
            ));
        };
        suffix = PathBuf::from(name).join(suffix);
        if !ancestor.pop() {
            return Err(err(
                ErrorCode::PathEscapesCwd,
                format!("path has no existing ancestor: {}", path.display()),
            ));
        }
    }

    if !ancestor.is_dir() {
        return Err(err(
            ErrorCode::NotAFile,
            format!("path ancestor is a file: {}", ancestor.display()),
        ));
    }

    let base = ancestor.canonicalize().map_err(|e| {
        err(
            ErrorCode::IoError,
            format!("failed to resolve {}: {e}", ancestor.display()),
        )
    })?;
    Ok(base.join(suffix))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

fn normalize_display(raw: &str) -> String {
    raw.replace('\\', "/")
}

fn path_key(path: &Path) -> String {
    let key = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        key.to_ascii_lowercase()
    } else {
        key
    }
}

fn err(code: ErrorCode, message: impl Into<String>) -> RearrangeError {
    RearrangeError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn resolves_missing_path_with_missing_parent() {
        let dir = tempdir().unwrap();
        let resolver = PathResolver::new(dir.path()).unwrap();
        let path = resolver.resolve("src/new.rs").unwrap();
        assert!(path.abs.ends_with("src/new.rs"));
    }

    #[test]
    fn rejects_escape() {
        let dir = tempdir().unwrap();
        let resolver = PathResolver::new(dir.path()).unwrap();
        let err = resolver.resolve("../outside.rs").unwrap_err();
        assert_eq!(err.code, ErrorCode::PathEscapesCwd);
    }

    #[test]
    fn rejects_file_ancestor() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("foo"), "x").unwrap();
        let resolver = PathResolver::new(dir.path()).unwrap();
        let err = resolver.resolve("foo/bar.rs").unwrap_err();
        assert_eq!(err.code, ErrorCode::NotAFile);
    }

    #[test]
    fn rejects_prefix_conflict() {
        let dir = tempdir().unwrap();
        let resolver = PathResolver::new(dir.path()).unwrap();
        let a = resolver.resolve("foo").unwrap();
        let b = resolver.resolve("foo/bar.rs").unwrap();
        let err = reject_prefix_conflicts(&[a, b]).unwrap_err();
        assert_eq!(err.code, ErrorCode::PathConflict);
    }
}
