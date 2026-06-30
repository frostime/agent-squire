use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::Serialize;

use super::model::{LineRange, Source};
use super::sources::{expand_dir, expand_glob, render_tree};

const LARGE_FILE_BYTES: u64 = 10 * 1024 * 1024;
const BINARY_CHECK_BYTES: usize = 8192;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_COMMAND_OUTPUT_BYTES: usize = 1_048_576;

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
    Cmd { command: String, in_zip: String },
    #[serde(rename_all = "camelCase")]
    Tree { path: String, in_zip: String },
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
    pub content: ArtifactContent,
}

#[derive(Debug, Clone)]
pub enum ArtifactContent {
    Text(String),
    Command(String),
}

struct CollectedEntries {
    file_entries: Vec<FileEntry>,
    manifest_entries: Vec<ManifestEntry>,
    artifact_entries: Vec<ArtifactEntry>,
    has_file_backed_entry: bool,
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

    // 1. Collect file entries + artifact descriptors. Command artifacts are
    // deferred until after validation and warning confirmation.
    let collected = collect_entries(sources, cwd, respect_gitignore)?;

    // Check: at least one file-backed source
    if !collected.has_file_backed_entry {
        bail!("No file sources to package. Use /list to review.");
    }

    // 2. Warnings check
    if !collect_warnings_and_confirm(&collected.file_entries)? {
        return Ok(None);
    }

    // 3. Resolve destination before staging so overwrite errors do not execute command artifacts.
    let default_name = format!("asq-gather-{}.zip", Utc::now().format("%Y%m%dT%H%M%S"));
    let dest = match output_path {
        Some(path) if path.is_absolute() => path,
        Some(path) => cwd.join(path),
        None => cwd.join(&default_name),
    };
    if dest.exists() {
        bail!("output file exists: {}", dest.display());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    // 4. Build staging directory
    let staging = assemble_staging_dir(
        &collected.file_entries,
        &collected.artifact_entries,
        &collected.manifest_entries,
        cwd,
    )?;

    // 5. Create zip
    let archive_temp = tempfile::tempdir()?;
    let staging_zip = archive_temp.path().join("output.zip");
    create_zip_archive(staging.path(), &staging_zip)?;

    // 5. Move to destination
    if fs::rename(&staging_zip, &dest).is_err() {
        fs::copy(&staging_zip, &dest)
            .with_context(|| format!("failed to copy zip to {}", dest.display()))?;
        let _ = fs::remove_file(&staging_zip);
    }

    println!(
        "  \u{2713} Zip written: {} ({} bytes)",
        dest.display(),
        dest.metadata().map(|m| m.len()).unwrap_or(0)
    );

    Ok(Some(dest))
}

pub fn collect_warnings_and_confirm(file_entries: &[FileEntry]) -> Result<bool> {
    let binaries: Vec<_> = file_entries.iter().filter(|e| e.is_binary).collect();
    let large: Vec<_> = file_entries
        .iter()
        .filter(|e| e.size > LARGE_FILE_BYTES)
        .collect();

    let has_warnings = !binaries.is_empty() || !large.is_empty();
    if !has_warnings {
        return Ok(true);
    }

    println!("  \u{26a0} Warnings:");
    if !binaries.is_empty() {
        println!("    {} binary file(s) detected:", binaries.len());
        for entry in &binaries {
            let note = if entry.size > LARGE_FILE_BYTES {
                " \u{2190} also exceeds 10 MB"
            } else {
                ""
            };
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
            bail!(
                "PowerShell Compress-Archive failed with exit code: {:?}",
                status.code()
            );
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
) -> Result<CollectedEntries> {
    let canonical_cwd = fs::canonicalize(cwd)
        .with_context(|| format!("failed to canonicalize cwd {}", cwd.display()))?;
    let mut file_entries: Vec<FileEntry> = Vec::new();
    let mut manifest_entries: Vec<ManifestEntry> = Vec::new();
    let mut artifact_entries: Vec<ArtifactEntry> = Vec::new();
    let mut has_file_backed_entry = false;
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
                        content: ArtifactContent::Text(content),
                    });
                    has_file_backed_entry = true;
                    manifest_entries.push(ManifestEntry::File {
                        path: display_path(path),
                        range: Some(RangeField {
                            start: range.start,
                            end: range.end,
                        }),
                        in_zip: format!("artifacts/{}", safe_name),
                    });
                    ranged_index += 1;
                } else {
                    // Full file
                    let entry = file_entry(cwd, &canonical_cwd, path)?;
                    let zip_path = entry.zip_path.clone();
                    file_entries.push(entry);
                    has_file_backed_entry = true;
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
                    let entry = file_entry(cwd, &canonical_cwd, f)?;
                    dir_files.push(entry.zip_path.clone());
                    file_entries.push(entry);
                }
                has_file_backed_entry |= !dir_files.is_empty();
                manifest_entries.push(ManifestEntry::Dir {
                    path: display_path(path),
                    file_count: dir_files.len(),
                    files: dir_files,
                });
            }

            Source::Tree { path } => {
                let text = render_tree(cwd, path, respect_gitignore)?;
                let safe_name = format!(
                    "tree-{}-{}.txt",
                    tree_index,
                    sanitize_filename(&display_path(path))
                );
                artifact_entries.push(ArtifactEntry {
                    zip_path: format!("artifacts/{}", safe_name),
                    content: ArtifactContent::Text(text),
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
                    let entry = file_entry(cwd, &canonical_cwd, f)?;
                    glob_files.push(entry.zip_path.clone());
                    file_entries.push(entry);
                }
                has_file_backed_entry |= !glob_files.is_empty();
                manifest_entries.push(ManifestEntry::Glob {
                    pattern: pattern.clone(),
                    file_count: glob_files.len(),
                    files: glob_files,
                });
            }

            Source::SelectedGlob { label, files } => {
                let mut glob_files: Vec<String> = Vec::new();
                for f in files {
                    let entry = file_entry(cwd, &canonical_cwd, f)?;
                    glob_files.push(entry.zip_path.clone());
                    file_entries.push(entry);
                }
                has_file_backed_entry |= !glob_files.is_empty();
                manifest_entries.push(ManifestEntry::Glob {
                    pattern: label.clone(),
                    file_count: glob_files.len(),
                    files: glob_files,
                });
            }

            Source::Command { command } => {
                let safe_name = format!("cmd-{}-{}.txt", cmd_index, sanitize_filename(command));
                artifact_entries.push(ArtifactEntry {
                    zip_path: format!("artifacts/{}", safe_name),
                    content: ArtifactContent::Command(command.clone()),
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

    Ok(CollectedEntries {
        file_entries,
        manifest_entries,
        artifact_entries,
        has_file_backed_entry,
    })
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
        fs::copy(&entry.disk_path, &dest).with_context(|| {
            format!(
                "failed to copy {} to {}",
                entry.disk_path.display(),
                dest.display()
            )
        })?;
    }

    // Write artifacts
    for entry in artifact_entries {
        let dest = staging.path().join(&entry.zip_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        match &entry.content {
            ArtifactContent::Text(content) => fs::write(&dest, content)?,
            ArtifactContent::Command(command) => fs::write(&dest, run_command(command, cwd)?)?,
        }
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

fn file_entry(cwd: &Path, canonical_cwd: &Path, path: &Path) -> Result<FileEntry> {
    let requested = resolve_path(cwd, path);
    let disk_path = fs::canonicalize(&requested)
        .with_context(|| format!("failed to canonicalize {}", requested.display()))?;
    let zip_path = zip_file_path(canonical_cwd, &disk_path)?;
    let size = disk_path
        .metadata()
        .with_context(|| format!("failed to stat {}", disk_path.display()))?
        .len();
    let is_binary = is_binary(&disk_path).unwrap_or(false);
    Ok(FileEntry {
        zip_path,
        disk_path,
        size,
        is_binary,
    })
}

/// Map a canonical disk path to its zip-internal path under `files/`.
fn zip_file_path(canonical_cwd: &Path, disk_path: &Path) -> Result<String> {
    if let Ok(relative) = disk_path.strip_prefix(canonical_cwd) {
        return Ok(format!("files/{}", safe_zip_relative_path(relative)?));
    }

    let safe = sanitize_filename(&display_path(disk_path));
    if safe.is_empty() {
        bail!(
            "cannot package external path with empty filename: {}",
            disk_path.display()
        );
    }
    Ok(format!("files/_external/{}", safe))
}

fn safe_zip_relative_path(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().replace('\\', "/")),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("unsafe zip path component in {}", path.display());
            }
        }
    }
    if parts.is_empty() {
        bail!("empty zip path for {}", path.display());
    }
    Ok(parts.join("/"))
}

fn read_file_range(path: &Path, range: LineRange) -> Result<String> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
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

fn run_command(command: &str, cwd: &Path) -> Result<String> {
    #[cfg(windows)]
    let mut command_process = {
        let mut process = std::process::Command::new("cmd");
        process.args(["/C", command]);
        process
    };
    #[cfg(not(windows))]
    let mut command_process = {
        let mut process = std::process::Command::new("sh");
        process.args(["-c", command]);
        process
    };

    command_process
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command_process
        .spawn()
        .with_context(|| format!("failed to execute: {}", command))?;

    let stdout = child.stdout.take().expect("stdout configured");
    let stderr = child.stderr.take().expect("stderr configured");
    let stdout_handle = thread::spawn(move || read_limited(stdout, MAX_COMMAND_OUTPUT_BYTES));
    let stderr_handle = thread::spawn(move || read_limited(stderr, MAX_COMMAND_OUTPUT_BYTES));

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= COMMAND_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "command timed out after {} seconds: {}",
                COMMAND_TIMEOUT.as_secs(),
                command
            );
        }
        thread::sleep(Duration::from_millis(20));
    };

    let stdout = join_limited_reader(stdout_handle, "stdout")?;
    let stderr = join_limited_reader(stderr_handle, "stderr")?;

    if !status.success() {
        bail!(
            "command failed (exit {}): {}\nstderr: {}",
            status.code().unwrap_or(-1),
            command,
            stream_text(&stderr).trim()
        );
    }

    Ok(stream_text(&stdout))
}

#[derive(Debug)]
struct LimitedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

fn read_limited<R: Read>(mut reader: R, limit: usize) -> Result<LimitedOutput> {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut buf = [0_u8; 8192];
    loop {
        let read = reader.read(&mut buf)?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        if remaining > 0 {
            let keep = remaining.min(read);
            bytes.extend_from_slice(&buf[..keep]);
        }
        if read > remaining {
            truncated = true;
        }
    }
    Ok(LimitedOutput { bytes, truncated })
}

fn join_limited_reader(
    handle: thread::JoinHandle<Result<LimitedOutput>>,
    stream: &str,
) -> Result<LimitedOutput> {
    handle
        .join()
        .map_err(|_| anyhow::anyhow!("command {stream} reader panicked"))?
}

fn stream_text(output: &LimitedOutput) -> String {
    let mut text = String::from_utf8_lossy(&output.bytes).into_owned();
    if output.truncated {
        text.push_str(&format!(
            "\n[truncated after {} bytes]\n",
            MAX_COMMAND_OUTPUT_BYTES
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_special_chars() {
        assert_eq!(sanitize_filename("src/main.rs"), "src-main.rs");
        assert_eq!(sanitize_filename("git diff HEAD~1"), "git-diff-HEAD~1");
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

    #[test]
    fn zip_file_path_keeps_inside_paths_relative() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        let file = dir.path().join("src/main.rs");
        fs::write(&file, "fn main() {}\n").unwrap();

        let path = zip_file_path(
            &fs::canonicalize(dir.path()).unwrap(),
            &fs::canonicalize(file).unwrap(),
        )
        .unwrap();
        assert_eq!(path, "files/src/main.rs");
    }

    #[test]
    fn zip_file_path_flattens_external_paths() {
        let cwd = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let file = external.path().join("outside.txt");
        fs::write(&file, "outside\n").unwrap();

        let path = zip_file_path(
            &fs::canonicalize(cwd.path()).unwrap(),
            &fs::canonicalize(file).unwrap(),
        )
        .unwrap();
        assert!(path.starts_with("files/_external/"), "{path}");
        assert!(!path.contains(".."), "{path}");
    }
}
