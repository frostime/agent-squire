use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::Serialize;

use super::model::{LineRange, Source};
use super::sources::{expand_dir, expand_glob, render_tree};

const LARGE_FILE_BYTES: u64 = 10 * 1024 * 1024;
const BINARY_CHECK_BYTES: usize = 8192;

// ── Manifest types ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema: u8,
    pub cwd: String,
    pub created: String,
    pub sources: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct RangeField {
    pub start: usize,
    pub end: usize,
}

// ── Internal assembly types ──

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

// ── Public entry point ──

pub fn assemble_zip(
    sources: &[Source],
    cwd: &Path,
    respect_gitignore: bool,
    output_path: Option<PathBuf>,
) -> Result<Option<PathBuf>> {
    if sources.is_empty() {
        bail!("No sources to package");
    }

    // 1. Collect file entries + artifacts
    let (file_entries, manifest_entries, artifact_entries) =
        collect_entries(sources, cwd, respect_gitignore)?;

    // Check: at least one file-backed source
    if file_entries.is_empty() {
        bail!("No file sources to package. Use /list to review.");
    }

    // 2. Warnings check
    if !collect_warnings_and_confirm(&file_entries)? {
        return Ok(None);
    }

    // 3. Build staging directory
    let staging = assemble_staging_dir(&file_entries, &artifact_entries, &manifest_entries, cwd)?;

    // 4. Create zip
    let default_name = format!(
        "asq-gather-{}.zip",
        Utc::now().format("%Y%m%dT%H%M%S")
    );
    let dest = output_path.unwrap_or_else(|| cwd.join(&default_name));
    let staging_zip = staging.path().join("output.zip");
    create_zip_archive(staging.path(), &staging_zip)?;

    // 5. Move to destination
    if fs::rename(&staging_zip, &dest).is_err() {
        fs::copy(&staging_zip, &dest)
            .with_context(|| format!("failed to copy zip to {}", dest.display()))?;
        let _ = fs::remove_file(&staging_zip);
    }

    println!("  \u{2713} Zip written: {} ({} bytes)", dest.display(), dest.metadata().map(|m| m.len()).unwrap_or(0));

    Ok(Some(dest))
}

pub fn collect_warnings_and_confirm(file_entries: &[FileEntry]) -> Result<bool> {
    let binaries: Vec<_> = file_entries.iter().filter(|e| e.is_binary).collect();
    let large: Vec<_> = file_entries.iter().filter(|e| e.size > LARGE_FILE_BYTES).collect();

    let has_warnings = !binaries.is_empty() || !large.is_empty();
    if !has_warnings {
        return Ok(true);
    }

    println!("  \u{26a0} Warnings:");
    if !binaries.is_empty() {
        println!("    {} binary file(s) detected:", binaries.len());
        for entry in &binaries {
            let note = if entry.size > LARGE_FILE_BYTES { " \u{2190} also exceeds 10 MB" } else { "" };
            println!("      - {} ({} bytes){}", entry.zip_path, entry.size, note);
        }
    }
    if !large.is_empty() {
        let large_only: Vec<_> = large.iter().filter(|e| !e.is_binary).collect();
        if !large_only.is_empty() {
            println!("    {} large file(s) (>10 MB):", large_only.len());
            for entry in &large_only {
                println!("      - {} ({} bytes)", entry.zip_path, entry.size);
            }
        }
    }
    print!("  Continue? [Y/n]: ");
    use std::io::{self, Write};
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(!answer.trim().eq_ignore_ascii_case("n"))
}

pub fn create_zip_archive(staging_dir: &Path, output: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        let status = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Compress-Archive",
                "-Path",
                &format!(r"{}\*", staging_dir.display()),
                "-DestinationPath",
                &output.display().to_string(),
                "-Force",
            ])
            .status()
            .context("PowerShell Compress-Archive not found. Is PowerShell available?")?;

        if !status.success() {
            bail!("PowerShell Compress-Archive failed with exit code: {:?}", status.code());
        }
    }

    #[cfg(not(windows))]
    {
        let status = std::process::Command::new("zip")
            .args(["-r", &output.display().to_string(), "."])
            .current_dir(staging_dir)
            .status()
            .context("zip not found. Install zip (apt install zip / brew install zip).")?;

        if !status.success() {
            bail!("zip command failed with exit code: {:?}", status.code());
        }
    }

    Ok(())
}

// ── Internal: entry collection ──

fn collect_entries(
    sources: &[Source],
    cwd: &Path,
    respect_gitignore: bool,
) -> Result<(Vec<FileEntry>, Vec<ManifestEntry>, Vec<ArtifactEntry>)> {
    let mut file_entries: Vec<FileEntry> = Vec::new();
    let mut manifest_entries: Vec<ManifestEntry> = Vec::new();
    let mut artifact_entries: Vec<ArtifactEntry> = Vec::new();
    let mut cmd_index: usize = 0;
    let mut tree_index: usize = 0;
    let mut ranged_index: usize = 0;

    for source in sources {
        match source {
            Source::File { path, range } => {
                if let Some(range) = range {
                    // Ranged file → artifact
                    let disk_path = resolve_path(cwd, path);
                    let content = read_file_range(&disk_path, *range)?;
                    let safe_name = format!(
                        "file-{}-{}-L{}-{}.txt",
                        ranged_index,
                        sanitize_filename(&display_path(path)),
                        range.start,
                        range.end
                    );
                    artifact_entries.push(ArtifactEntry {
                        zip_path: format!("artifacts/{}", safe_name),
                        content,
                    });
                    manifest_entries.push(ManifestEntry::File {
                        path: display_path(path),
                        range: Some(RangeField { start: range.start, end: range.end }),
                        in_zip: format!("artifacts/{}", safe_name),
                    });
                    ranged_index += 1;
                } else {
                    // Full file
                    let disk_path = resolve_path(cwd, path);
                    let zip_path = zip_file_path(path)?;
                    let size = disk_path.metadata().map(|m| m.len()).unwrap_or(0);
                    let binary = is_binary(&disk_path).unwrap_or(false);
                    file_entries.push(FileEntry {
                        zip_path: zip_path.clone(),
                        disk_path,
                        size,
                        is_binary: binary,
                    });
                    manifest_entries.push(ManifestEntry::File {
                        path: display_path(path),
                        range: None,
                        in_zip: zip_path,
                    });
                }
            }

            Source::Dir { path } => {
                let files = expand_dir(cwd, path, respect_gitignore)?;
                let mut dir_files: Vec<String> = Vec::new();
                for f in &files {
                    let disk_path = resolve_path(cwd, f);
                    let zip_path = zip_file_path(f)?;
                    let size = disk_path.metadata().map(|m| m.len()).unwrap_or(0);
                    let binary = is_binary(&disk_path).unwrap_or(false);
                    dir_files.push(zip_path.clone());
                    file_entries.push(FileEntry {
                        zip_path,
                        disk_path,
                        size,
                        is_binary: binary,
                    });
                }
                manifest_entries.push(ManifestEntry::Dir {
                    path: display_path(path),
                    file_count: dir_files.len(),
                    files: dir_files,
                });
            }

            Source::Tree { path } => {
                let text = render_tree(cwd, path, respect_gitignore)?;
                let safe_name = format!("tree-{}-{}.txt", tree_index, sanitize_filename(&display_path(path)));
                artifact_entries.push(ArtifactEntry {
                    zip_path: format!("artifacts/{}", safe_name),
                    content: text,
                });
                manifest_entries.push(ManifestEntry::Tree {
                    path: display_path(path),
                    in_zip: format!("artifacts/{}", safe_name),
                });
                tree_index += 1;
            }

            Source::Glob { pattern } => {
                let files = expand_glob(cwd, pattern)?;
                let mut glob_files: Vec<String> = Vec::new();
                for f in &files {
                    let disk_path = resolve_path(cwd, f);
                    let zip_path = zip_file_path(f)?;
                    let size = disk_path.metadata().map(|m| m.len()).unwrap_or(0);
                    let binary = is_binary(&disk_path).unwrap_or(false);
                    glob_files.push(zip_path.clone());
                    file_entries.push(FileEntry {
                        zip_path,
                        disk_path,
                        size,
                        is_binary: binary,
                    });
                }
                manifest_entries.push(ManifestEntry::Glob {
                    pattern: pattern.clone(),
                    file_count: glob_files.len(),
                    files: glob_files,
                });
            }

            Source::SelectedGlob { label, files } => {
                let mut glob_files: Vec<String> = Vec::new();
                for f in files {
                    let disk_path = resolve_path(cwd, f);
                    let zip_path = zip_file_path(f)?;
                    let size = disk_path.metadata().map(|m| m.len()).unwrap_or(0);
                    let binary = is_binary(&disk_path).unwrap_or(false);
                    glob_files.push(zip_path.clone());
                    file_entries.push(FileEntry {
                        zip_path,
                        disk_path,
                        size,
                        is_binary: binary,
                    });
                }
                manifest_entries.push(ManifestEntry::Glob {
                    pattern: label.clone(),
                    file_count: glob_files.len(),
                    files: glob_files,
                });
            }

            Source::Command { command } => {
                let output = run_command(command)?;
                let safe_name = format!("cmd-{}-{}.txt", cmd_index, sanitize_filename(command));
                artifact_entries.push(ArtifactEntry {
                    zip_path: format!("artifacts/{}", safe_name),
                    content: output,
                });
                manifest_entries.push(ManifestEntry::Cmd {
                    command: command.clone(),
                    in_zip: format!("artifacts/{}", safe_name),
                });
                cmd_index += 1;
            }
        }
    }

    // Dedup file entries by zip_path
    file_entries = dedup_files(file_entries);

    Ok((file_entries, manifest_entries, artifact_entries))
}

// ── Internal: staging dir assembly ──

fn assemble_staging_dir(
    file_entries: &[FileEntry],
    artifact_entries: &[ArtifactEntry],
    manifest_entries: &[ManifestEntry],
    cwd: &Path,
) -> Result<tempfile::TempDir> {
    let staging = tempfile::tempdir()?;

    // Create subdirs
    fs::create_dir_all(staging.path().join("files"))?;
    if !artifact_entries.is_empty() {
        fs::create_dir_all(staging.path().join("artifacts"))?;
    }

    // Copy files
    for entry in file_entries {
        let dest = staging.path().join(&entry.zip_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&entry.disk_path, &dest)
            .with_context(|| format!("failed to copy {} to {}", entry.disk_path.display(), dest.display()))?;
    }

    // Write artifacts
    for entry in artifact_entries {
        let dest = staging.path().join(&entry.zip_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, &entry.content)?;
    }

    // Write manifest.json
    let manifest = Manifest {
        schema: 1,
        cwd: cwd.display().to_string().replace('\\', "/"),
        created: Utc::now().to_rfc3339(),
        sources: manifest_entries.to_vec(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    fs::write(staging.path().join("manifest.json"), manifest_json)?;

    Ok(staging)
}

// ── Helpers ──

pub fn sanitize_filename(raw: &str) -> String {
    raw.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|', ' '], "-")
        .trim_matches('-')
        .to_string()
}

fn dedup_files(entries: Vec<FileEntry>) -> Vec<FileEntry> {
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .filter(|e| seen.insert(e.zip_path.clone()))
        .collect()
}

fn is_binary(path: &Path) -> Result<bool> {
    let mut f = fs::File::open(path)?;
    let mut buf = vec![0u8; BINARY_CHECK_BYTES];
    let n = f.read(&mut buf)?;
    Ok(buf[..n].contains(&0))
}

fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn resolve_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

/// Map a relative file path to its zip-internal path under `files/`.
fn zip_file_path(path: &Path) -> Result<String> {
    let normalized = display_path(path);
    // Prevent path traversal
    if normalized.starts_with("..") || Path::new(&normalized).is_absolute() {
        // External file: flatten to safe name
        let safe = sanitize_filename(&normalized);
        return Ok(format!("files/_external/{}", safe));
    }
    Ok(format!("files/{}", normalized))
}

fn read_file_range(path: &Path, range: LineRange) -> Result<String> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let lines: Vec<&str> = content.lines().collect();
    let start = range.start.saturating_sub(1);
    let end = range.end.min(lines.len());
    if start >= lines.len() {
        bail!(
            "range {}-{} out of bounds: file {} has {} lines",
            range.start,
            range.end,
            path.display(),
            lines.len()
        );
    }
    Ok(lines[start..end].join("\n"))
}

fn run_command(command: &str) -> Result<String> {
    #[cfg(windows)]
    let output = {
        std::process::Command::new("cmd")
            .args(["/C", command])
            .output()
            .with_context(|| format!("failed to execute: {}", command))?
    };
    #[cfg(not(windows))]
    let output = {
        std::process::Command::new("sh")
            .args(["-c", command])
            .output()
            .with_context(|| format!("failed to execute: {}", command))?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "command failed (exit {}): {}\nstderr: {}",
            output.status.code().unwrap_or(-1),
            command,
            stderr.trim()
        );
    }

    let stdout = String::from_utf8(output.stdout)
        .context("command output was not valid UTF-8")?;
    Ok(stdout)
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

    #[test]
    fn read_range_slices_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "one\ntwo\nthree\nfour\nfive\n").unwrap();

        let result = read_file_range(&path, LineRange::new(2, 4).unwrap()).unwrap();
        assert_eq!(result, "two\nthree\nfour");
    }
}
