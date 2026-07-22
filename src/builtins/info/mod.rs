use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Local, SecondsFormat};
use clap::Args;
use encoding_rs::{GBK, UTF_16BE, UTF_16LE};
use serde::Serialize;

use crate::builtins::source::{self, Dedup, GitignoreMode, SourcePolicy};
use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

const TEXT_SAMPLE_BYTES: u64 = 65_536;
const LINE_COUNT_LIMIT: u64 = 1024 * 1024;

#[derive(Args, Debug)]
#[command(
    long_about = r#"Inspect file metadata and text/binary format without printing file contents.

Use this when an agent needs to decide whether a file is safe/useful to read: size, binary/text kind, detected encoding, newline style, BOM, modified time, and line count when available. It accepts files, directories, and glob patterns; directories are expanded recursively.

This command does not search inside files and does not summarize content. Use `rg` for search, `md-toc` for Markdown heading navigation, and `read-range` to read selected line ranges."#,
    after_help = r#"Examples:
    asq file-info README.md src/cli.rs
    asq file-info src --max-files 20
    asq file-info "docs/**/*.md"
    asq --print json file-info README.md"#
)]
pub struct InfoArgs {
    #[arg(help = "Files, directories, or glob patterns to inspect")]
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

    let (mut paths, missing) = resolve_sources(&args.sources, args.max_files)?;
    paths.sort();
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
            let payload = Envelope::new("info", data);
            output::print_json(&payload)?;
        }
        _ => {
            print_compact_info(&infos, &missing, &ctx.cwd);
        }
    }

    Ok(0)
}

fn resolve_sources(
    sources: &[String],
    max_files: Option<usize>,
) -> Result<(Vec<PathBuf>, Vec<String>)> {
    // `~`-expansion matches the original `file-info` behavior: only apply it
    // to non-glob inputs (legacy code expanded for the file/dir existence
    // test but kept raw glob patterns). Glob-magic inputs therefore pass
    // through verbatim, exactly as before.
    let expanded: Vec<String> = sources
        .iter()
        .map(|s| {
            if source::has_glob_magic(s) {
                s.clone()
            } else {
                expand_home(s)
            }
        })
        .collect();

    // Eagerly canonicalize paths inside `map` so the dedup key (Canonicalize)
    // and downstream `inspect_file` operate on absolute, normalized paths.
    // The wrapper sorts the result lexicographically afterward to preserve
    // the previous BTreeMap output order.
    let (paths, missing) = source::resolve(
        &expanded,
        SourcePolicy {
            root: Path::new("."),
            gitignore: GitignoreMode::Off,
            accept_file: &|_| true,
            filter_explicit_file: false,
            filter_glob: false,
            dedup: Dedup::Canonicalize,
            max_files,
            map: &|p, _| Some(p.canonicalize().unwrap_or(p)),
        },
    )?;
    Ok((paths, missing))
}

fn read_sample(path: &Path, size: u64) -> Result<Vec<u8>> {
    use std::io::Read;
    let read_len = size.min(TEXT_SAMPLE_BYTES) as usize;
    let mut buf = vec![0u8; read_len];
    let mut f =
        fs::File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
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
    let newline = detect_newline(&sample, &encoding, is_binary);
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
    let bom = crate::runtime::encoding::detect_bom(data);
    match bom {
        crate::runtime::encoding::Bom::None => ("none".into(), None),
        other => {
            let label = other.label().to_string();
            (label.clone(), Some(label))
        }
    }
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

// SPEC: File metadata reports newline style of decoded text, not byte patterns.
// Raw-byte scanning misclassifies UTF-16 CRLF as mixed.
fn detect_newline(data: &[u8], encoding: &str, is_binary: bool) -> String {
    if is_binary || data.is_empty() {
        return "unknown".into();
    }

    if let Some(text) = decode_sample_text(data, encoding) {
        return crate::runtime::encoding::detect_newline_text(&text)
            .label()
            .into();
    }

    "unknown".into()
}

fn decode_sample_text(data: &[u8], encoding: &str) -> Option<String> {
    match encoding {
        "utf-8" => std::str::from_utf8(data).ok().map(ToOwned::to_owned),
        "utf-8-sig" => std::str::from_utf8(data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data))
            .ok()
            .map(ToOwned::to_owned),
        "utf-16-le" => {
            let data = data.strip_prefix(&[0xFF, 0xFE]).unwrap_or(data);
            let (text, _, had_errors) = UTF_16LE.decode(data);
            (!had_errors).then(|| text.into_owned())
        }
        "utf-16-be" => {
            let data = data.strip_prefix(&[0xFE, 0xFF]).unwrap_or(data);
            let (text, _, had_errors) = UTF_16BE.decode(data);
            (!had_errors).then(|| text.into_owned())
        }
        "gbk" => {
            let (text, _, had_errors) = GBK.decode(data);
            (!had_errors).then(|| text.into_owned())
        }
        "latin1" => Some(data.iter().map(|b| *b as char).collect()),
        _ => None,
    }
}

// SPEC: line_count counts logical decoded-text lines. Single-byte encodings may
// be counted from bytes; UTF-16 must be decoded before counting separators.
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
        "utf-8" => {
            std::str::from_utf8(&raw).ok()?;
        }
        "utf-8-sig" => {
            let data = raw.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&raw);
            std::str::from_utf8(data).ok()?;
        }
        "utf-16-le" | "utf-16-be" => {
            let text = decode_sample_text(&raw, encoding)?;
            return Some(count_text_lines(&text));
        }
        "gbk" => {
            let (_, _, had_errors) = GBK.decode(&raw);
            if had_errors {
                return None;
            }
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
    Some(if ends_with_newline {
        separators
    } else {
        separators + 1
    })
}

fn count_text_lines(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    let raw = text.as_bytes();
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
    if last == Some(b'\n') || last == Some(b'\r') {
        separators
    } else {
        separators + 1
    }
}

fn display_path(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    crate::runtime::pathutil::display_relative(path, &cwd)
}

fn expand_home(source: &str) -> String {
    if let Some(rest) = source.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))
    {
        return format!("{home}/{rest}");
    }
    source.to_string()
}

fn print_compact_info(infos: &[FileInfo], missing: &[String], cwd: &Path) {
    let cwd_str = cwd.to_string_lossy();
    for info in infos {
        let display_path = make_relative(&info.path, &cwd_str);
        let size = format_size(info.size_bytes);
        if info.kind == "binary" {
            println!("{display_path} | {size} | binary");
        } else {
            let mut parts = vec![display_path, size, info.encoding.clone()];
            if info.bom != "none" {
                parts.push(format!("bom:{}", info.bom));
            }
            parts.push(info.newline.clone());
            if let Some(n) = info.line_count {
                parts.push(format!("{n}L"));
            }
            println!("{}", parts.join(" | "));
        }
    }
    if !missing.is_empty() {
        println!("missing: {}", missing.join(", "));
    }
}

fn make_relative(path: &str, cwd: &str) -> String {
    // Strip Windows extended-length prefix
    let clean = path.strip_prefix(r"//?/").unwrap_or(path);
    let clean = clean.strip_prefix(r"\\?\").unwrap_or(clean);
    // Try to make relative to cwd
    let cwd_clean = cwd.strip_prefix(r"//?/").unwrap_or(cwd);
    let cwd_clean = cwd_clean.strip_prefix(r"\\?\").unwrap_or(cwd_clean);
    // Normalize separators for comparison
    let norm_path = clean.replace('\\', "/");
    let norm_cwd = cwd_clean.replace('\\', "/");
    let prefix = if norm_cwd.ends_with('/') {
        norm_cwd.clone()
    } else {
        format!("{norm_cwd}/")
    };
    if let Some(rel) = norm_path.strip_prefix(&prefix) {
        rel.to_string()
    } else {
        norm_path
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
