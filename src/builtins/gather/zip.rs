use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::Serialize;

use super::model::{LineRange, Source};

const LARGE_FILE_BYTES: u64 = 10 * 1024 * 1024;
const BINARY_CHECK_BYTES: usize = 8192;

// Manifest types

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema: u8,
    pub cwd: String,
    pub created: String,
    pub sources: Vec<ManifestEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ManifestEntry {
    #[serde(rename_all = "camelCase")]
    File {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<RangeField>,
        in_zip: String,
    },
    #[serde(rename_all = "camelCase")]
    Dir {
        path: String,
        file_count: usize,
        files: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    Glob {
        pattern: String,
        file_count: usize,
        files: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    Cmd {
        command: String,
        in_zip: String,
    },
    #[serde(rename_all = "camelCase")]
    Tree {
        path: String,
        in_zip: String,
    },
}

#[derive(Debug, Serialize)]
pub struct RangeField {
    pub start: usize,
    pub end: usize,
}

// Internal assembly types

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub zip_path: String,
    pub disk_path: PathBuf,
    pub size: u64,
    pub is_binary: bool,
}

#[derive(Debug, Clone)]
pub struct ArtifactEntry {
    pub zip_path: String,
    pub content: String,
}

#[allow(dead_code)]
enum ArtifactMeta {
    Cmd { command: String, index: usize },
    Tree { path: PathBuf, index: usize },
    RangedFile { original_path: PathBuf, range: LineRange, index: usize },
}

#[allow(dead_code)]
enum Warning {
    Binary { path: String, size: u64 },
    LargeFile { path: String, size: u64 },
    ExternalPath { path: String, zip_path: String },
}

// Public entry point

pub fn assemble_zip(
    sources: &[Source],
    cwd: &Path,
    respect_gitignore: bool,
    output_path: Option<PathBuf>,
) -> Result<Option<PathBuf>> {
    let _ = (sources, cwd, respect_gitignore, output_path);
    bail!("not yet implemented")
}

pub fn collect_warnings_and_confirm(file_entries: &[FileEntry]) -> Result<bool> {
    let _ = file_entries;
    bail!("not yet implemented")
}

pub fn create_zip_archive(staging_dir: &Path, output: &Path) -> Result<()> {
    let _ = (staging_dir, output);
    bail!("not yet implemented")
}

// Helpers

pub fn sanitize_filename(raw: &str) -> String {
    raw.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|', ' '], "-")
        .trim_matches('-')
        .to_string()
}

#[allow(dead_code)]
fn dedup_files(entries: Vec<FileEntry>) -> Vec<FileEntry> {
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .filter(|e| seen.insert(e.zip_path.clone()))
        .collect()
}

#[allow(dead_code)]
fn is_binary(path: &Path) -> Result<bool> {
    let mut f = fs::File::open(path)?;
    let mut buf = vec![0u8; BINARY_CHECK_BYTES];
    let n = f.read(&mut buf)?;
    Ok(buf[..n].contains(&0))
}

#[allow(dead_code)]
fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_special_chars() {
        assert_eq!(sanitize_filename("src/main.rs"), "src-main.rs");
        assert_eq!(
            sanitize_filename("git diff HEAD~1"),
            "git-diff-HEAD~1"
        );
        assert_eq!(
            sanitize_filename("C:\\Users\\a.rs:10-20"),
            "C--Users-a.rs-10-20"
        );
    }
}
