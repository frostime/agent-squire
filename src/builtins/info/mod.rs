use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Local, SecondsFormat};
use clap::Args;
use encoding_rs::GBK;
use glob::glob;
use serde::Serialize;
use walkdir::WalkDir;

use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

const TEXT_SAMPLE_BYTES: u64 = 65_536;
const LINE_COUNT_LIMIT: u64 = 1024 * 1024;
const GLOB_CHARS: &[char] = &['*', '?', '['];

#[derive(Args, Debug)]
#[command(
    long_about = "Inspect one or more files. Supports files, directories, and glob patterns. Directories are expanded recursively."
)]
pub struct InfoArgs {
    #[arg(help = "Files, directories, or glob patterns")]
    pub sources: Vec<String>,

    #[arg(long, value_name = "N", help = "Maximum number of files to inspect")]
    pub max_files: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    path: String,
    size_bytes: u64,
    modified: String,
    kind: String,
    encoding: String,
    bom: String,
    newline: String,
    line_count: Option<usize>,
}

#[derive(Debug, Serialize)]
struct InfoData {
    files: Vec<FileInfo>,
    count: usize,
    missing_sources: Vec<String>,
}

pub fn run(args: InfoArgs, ctx: &CommandContext) -> Result<u8> {
    if args.max_files == Some(0) {
        anyhow::bail!("--max-files must be >= 1");
    }

    let (paths, missing) = resolve_sources(&args.sources, args.max_files)?;
    if paths.is_empty() && !missing.is_empty() {
        anyhow::bail!("No files found for the provided inputs.");
    }

    if paths.is_empty() {
        println!("No files found.");
        return Ok(0);
    }

    let infos = paths
        .iter()
        .map(|path| inspect_file(path))
        .collect::<Result<Vec<_>>>()?;

    match ctx.print {
        PrintMode::Json => {
            let data = InfoData {
                count: infos.len(),
                files: infos,
                missing_sources: missing,
            };
            let payload = Envelope {
                ok: true,
                command: "info",
                data,
                warnings: vec![],
                meta: serde_json::json!({}),
            };
            output::print_json(&payload)?;
        }
        _ => {
            println!("File Info ({} file(s))", infos.len());
            println!(
                "{:<42} {:<7} {:>10} {:<12} {:<10} {:<8} {:>8}",
                "Path", "Kind", "Size", "Encoding", "BOM", "Newline", "Lines"
            );
            for info in &infos {
                println!(
                    "{:<42} {:<7} {:>10} {:<12} {:<10} {:<8} {:>8}",
                    truncate(&info.path, 42),
                    info.kind,
                    info.size_bytes,
                    info.encoding,
                    info.bom,
                    info.newline,
                    info.line_count
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "-".into())
                );
            }
            if !missing.is_empty() {
                println!("Missing sources: {}", missing.join(", "));
            }
        }
    }

    Ok(0)
}

fn resolve_sources(
    sources: &[String],
    max_files: Option<usize>,
) -> Result<(Vec<PathBuf>, Vec<String>)> {
    let effective = if sources.is_empty() {
        vec![".".to_string()]
    } else {
        sources.to_vec()
    };
    let mut ordered: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    let mut missing = Vec::new();

    for source in effective {
        if max_files.is_some_and(|n| ordered.len() >= n) {
            break;
        }

        let expanded = expand_home(&source);
        let path = PathBuf::from(&expanded);

        if path.exists() {
            add_existing(&mut ordered, &path, max_files)?;
            continue;
        }

        if has_glob_magic(&source) {
            let mut any = false;
            for entry in glob(&source).with_context(|| format!("invalid glob pattern: {source}"))? {
                let path = entry?;
                if path.exists() {
                    any = true;
                    add_existing(&mut ordered, &path, max_files)?;
                    if max_files.is_some_and(|n| ordered.len() >= n) {
                        break;
                    }
                }
            }
            if !any {
                missing.push(source);
            }
            continue;
        }

        missing.push(source);
    }

    Ok((ordered.into_values().collect(), missing))
}

fn add_existing(
    ordered: &mut BTreeMap<PathBuf, PathBuf>,
    path: &Path,
    max_files: Option<usize>,
) -> Result<()> {
    if max_files.is_some_and(|n| ordered.len() >= n) {
        return Ok(());
    }

    if path.is_file() {
        let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        ordered.entry(resolved.clone()).or_insert(resolved);
        return Ok(());
    }

    if path.is_dir() {
        for entry in WalkDir::new(path)
            .sort_by_file_name()
            .into_iter()
            .filter_map(Result::ok)
        {
            if max_files.is_some_and(|n| ordered.len() >= n) {
                break;
            }
            let p = entry.path();
            if p.is_file() {
                let resolved = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
                ordered.entry(resolved.clone()).or_insert(resolved);
            }
        }
    }

    Ok(())
}

fn read_sample(path: &Path, size: u64) -> Result<Vec<u8>> {
    use std::io::Read;
    let read_len = size.min(TEXT_SAMPLE_BYTES) as usize;
    let mut buf = vec![0u8; read_len];
    let mut f = fs::File::open(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    f.read_exact(&mut buf)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(buf)
}

fn inspect_file(path: &Path) -> Result<FileInfo> {
    let stat = fs::metadata(path).with_context(|| format!("failed to stat {}", path.display()))?;
    let sample = read_sample(path, stat.len())?;

    let (bom, bom_encoding) = detect_bom(&sample);
    let is_binary = is_binary_data(&sample, bom_encoding.as_deref());
    let encoding = guess_encoding(&sample, bom_encoding.as_deref(), is_binary);
    let newline = detect_newline(&sample, is_binary);
    let line_count = count_lines(path, &encoding, stat.len(), is_binary);

    let modified: DateTime<Local> = stat
        .modified()
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        .into();

    Ok(FileInfo {
        path: display_path(path),
        size_bytes: stat.len(),
        modified: modified.to_rfc3339_opts(SecondsFormat::Secs, false),
        kind: if is_binary {
            "binary".into()
        } else {
            "text".into()
        },
        encoding,
        bom,
        newline,
        line_count,
    })
}

fn detect_bom(data: &[u8]) -> (String, Option<String>) {
    if data.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return ("utf-8-sig".into(), Some("utf-8-sig".into()));
    }
    if data.starts_with(&[0xFF, 0xFE]) {
        return ("utf-16-le".into(), Some("utf-16-le".into()));
    }
    if data.starts_with(&[0xFE, 0xFF]) {
        return ("utf-16-be".into(), Some("utf-16-be".into()));
    }
    ("none".into(), None)
}

fn is_binary_data(data: &[u8], bom_encoding: Option<&str>) -> bool {
    if data.is_empty() || bom_encoding.is_some() {
        return false;
    }
    if data.contains(&0) {
        return true;
    }

    let disallowed = data
        .iter()
        .filter(|byte| {
            let b = **byte;
            !(b == 9 || b == 10 || b == 13 || (32..=126).contains(&b) || b >= 128)
        })
        .count();

    disallowed as f64 / data.len().max(1) as f64 > 0.30
}

fn guess_encoding(data: &[u8], bom_encoding: Option<&str>, is_binary: bool) -> String {
    if let Some(enc) = bom_encoding {
        return enc.to_string();
    }
    if is_binary {
        return "binary".into();
    }
    if std::str::from_utf8(data).is_ok() {
        return "utf-8".into();
    }
    let (_, _, had_errors) = GBK.decode(data);
    if !had_errors {
        return "gbk".into();
    }
    // latin1 accepts all byte sequences — use it as final fallback (matches Python behavior)
    "latin1".into()
}

fn detect_newline(data: &[u8], is_binary: bool) -> String {
    if is_binary || data.is_empty() {
        return "unknown".into();
    }

    let has_crlf = data.windows(2).any(|w| w == b"\r\n");
    // Strip CRLF pairs, then check for standalone LF / CR
    let stripped: Vec<u8> = {
        let mut out = Vec::with_capacity(data.len());
        let mut i = 0;
        while i < data.len() {
            if i + 1 < data.len() && data[i] == b'\r' && data[i + 1] == b'\n' {
                i += 2;
            } else {
                out.push(data[i]);
                i += 1;
            }
        }
        out
    };

    let has_lf = stripped.contains(&b'\n');
    let has_cr = stripped.contains(&b'\r');

    match (has_crlf, has_lf, has_cr) {
        (true, false, false) => "crlf".into(),
        (false, true, false) => "lf".into(),
        (false, false, true) => "cr".into(),
        (false, false, false) => "none".into(),
        _ => "mixed".into(),
    }
}

fn count_lines(path: &Path, encoding: &str, size_bytes: u64, is_binary: bool) -> Option<usize> {
    if is_binary || encoding == "binary" || encoding == "unknown" || size_bytes > LINE_COUNT_LIMIT {
        return None;
    }

    let raw = fs::read(path).ok()?;

    // For line counting we only need to find \n and \r bytes.
    // For latin1/utf-8/gbk these are always single-byte, so we can count from raw bytes directly.
    if raw.is_empty() {
        return Some(0);
    }

    // Validate decoding (to confirm it's actually the claimed encoding)
    match encoding {
        "utf-8" => { std::str::from_utf8(&raw).ok()?; }
        "utf-8-sig" => {
            let data = raw.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&raw);
            std::str::from_utf8(data).ok()?;
        }
        "gbk" => {
            let (_, _, had_errors) = GBK.decode(&raw);
            if had_errors { return None; }
        }
        "latin1" => {} // all byte sequences are valid latin1
        _ => return None,
    }

    // Count line separators: \n, \r\n, standalone \r
    let mut separators = 0usize;
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'\r' {
            separators += 1;
            if i + 1 < raw.len() && raw[i + 1] == b'\n' {
                i += 2;
            } else {
                i += 1;
            }
        } else if raw[i] == b'\n' {
            separators += 1;
            i += 1;
        } else {
            i += 1;
        }
    }

    let last = raw.last().copied();
    let ends_with_newline = last == Some(b'\n') || last == Some(b'\r');
    Some(if ends_with_newline { separators } else { separators + 1 })
}

fn display_path(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.strip_prefix(&cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn has_glob_magic(source: &str) -> bool {
    source.chars().any(|c| GLOB_CHARS.contains(&c))
}

fn expand_home(source: &str) -> String {
    if let Some(rest) = source.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
        {
            return format!("{home}/{rest}");
        }
    }
    source.to_string()
}

fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_string();
    }
    let mut out = s.chars().take(width.saturating_sub(1)).collect::<String>();
    out.push('…');
    out
}
