use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Args;
use encoding_rs::GBK;
use serde::Serialize;

use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

const LONG_ABOUT: &str = r#"Read known 1-based line ranges from one text file.

Use this when an agent already knows the exact line numbers to inspect and needs a cross-platform replacement for `sed -n` / PowerShell line slicing. It does not search text, parse syntax, or find symbols; use `rg`, `md-toc`, or language tooling for discovery first.

Range syntax is 1-based and inclusive:
  N        one line
  A-B      range from A through B
  N~K      K lines before and after N

N/A/B may be a positive integer, `start`, or `end`. Repeat --range to read multiple ranges; request order and duplicates are preserved.

Without --range/--head/--tail, the entire file is printed."#;

#[derive(Args, Debug)]
#[command(
    long_about = LONG_ABOUT,
    after_help = r#"Examples:
  squire range src/cli.rs -r 10
  squire range src/cli.rs -r 10-30
  squire range src/cli.rs -r 120~20
  squire range src/cli.rs -r start-80 -r 120-end
  squire range src/cli.rs --head 20
  squire range src/cli.rs --tail 30
  squire range src/cli.rs                 # entire file
  squire --print json range src/cli.rs -r 10-30"#)]
pub struct ReadRangeArgs {
    #[arg(help = "Text file to read")]
    pub file: PathBuf,

    #[arg(
        short = 'r',
        long = "range",
        value_name = "RANGE",
        help = "Repeatable 1-based range: N, A-B, or N~K; supports start/end",
        long_help = "Repeatable 1-based inclusive line range selector. Forms: N, A-B, N~K. N/A/B may be a positive integer, start, or end. K must be a non-negative integer."
    )]
    pub ranges: Vec<String>,

    #[arg(
        long,
        value_name = "N",
        help = "Read first N lines (mutually exclusive with --range/--tail)"
    )]
    pub head: Option<usize>,

    #[arg(
        long,
        value_name = "N",
        help = "Read last N lines (mutually exclusive with --range/--head)"
    )]
    pub tail: Option<usize>,

    #[arg(
        long = "no-number",
        default_value = "false",
        help = "Do not display line numbers in output"
    )]
    pub no_number: bool,
}

#[derive(Debug, Serialize)]
struct ReadRangeData {
    file: ReadRangeFile,
    slices: Vec<ReadRangeSlice>,
}

#[derive(Debug, Serialize)]
struct ReadRangeFile {
    path: String,
    encoding: String,
    newline: String,
    line_count: usize,
}

#[derive(Debug, Serialize)]
struct ReadRangeSlice {
    request: String,
    start_line: usize,
    end_line: usize,
    content: String,
}

#[derive(Debug, Clone)]
enum Point {
    Start,
    End,
    Line(usize),
}

#[derive(Debug, Clone)]
enum RangeSpec {
    One(Point),
    Range(Point, Point),
    Context(Point, usize),
}

#[derive(Debug)]
struct ResolvedRange {
    request: String,
    start_line: usize,
    end_line: usize,
    clipped: bool,
}

struct TextFile {
    path: String,
    encoding: String,
    newline: String,
    lines: Vec<String>,
}

pub fn run(args: ReadRangeArgs, ctx: &CommandContext) -> Result<u8> {
    let has_range = !args.ranges.is_empty();
    let has_head = args.head.is_some();
    let has_tail = args.tail.is_some();
    let no_number = args.no_number;

    let count = [has_range, has_head, has_tail]
        .iter()
        .filter(|&&b| b)
        .count();
    if count > 1 {
        bail!("--range, --head, and --tail are mutually exclusive");
    }

    let text = read_text_file(&args.file)?;
    if text.lines.is_empty() {
        bail!("{} has no lines", text.path);
    }

    let (specs, display_requests): (Vec<_>, Vec<String>) = if let Some(n) = args.head {
        if n == 0 {
            bail!("--head must be at least 1");
        }
        let spec = RangeSpec::Range(Point::Start, Point::Line(n));
        (vec![spec], vec![format!("head:{n}")])
    } else if let Some(n) = args.tail {
        if n == 0 {
            bail!("--tail must be at least 1");
        }
        (vec![RangeSpec::One(Point::End)], vec![format!("tail:{n}")])
    } else if has_range {
        let specs = args
            .ranges
            .iter()
            .map(|spec| parse_range(spec))
            .collect::<Result<Vec<_>>>()?;
        (specs, args.ranges.clone())
    } else {
        // no args: entire file
        (
            vec![RangeSpec::Range(Point::Start, Point::End)],
            vec!["all".into()],
        )
    };

    let requests: Vec<&str> = display_requests.iter().map(|s| s.as_str()).collect();
    let mut warnings = Vec::new();
    let resolved = specs
        .iter()
        .zip(requests.iter())
        .map(|(spec, &request)| {
            let mut r = resolve_range(spec, request, text.lines.len())?;
            if let Some(n) = args.tail {
                let start = text.lines.len().saturating_sub(n) + 1;
                r.start_line = start.max(1);
                r.end_line = text.lines.len();
                r.clipped = false;
            }
            Ok(r)
        })
        .collect::<Result<Vec<_>>>()?;

    let slices = resolved
        .iter()
        .map(|slice| {
            if slice.clipped {
                warnings.push(format!(
                    "range {} clipped to {}-{}",
                    slice.request, slice.start_line, slice.end_line
                ));
            }
            ReadRangeSlice {
                request: slice.request.clone(),
                start_line: slice.start_line,
                end_line: slice.end_line,
                content: text.lines[(slice.start_line - 1)..slice.end_line].join("\n"),
            }
        })
        .collect::<Vec<_>>();

    let file = ReadRangeFile {
        path: text.path,
        encoding: text.encoding,
        newline: text.newline,
        line_count: text.lines.len(),
    };

    match ctx.print {
        PrintMode::Json => {
            let payload = Envelope {
                ok: true,
                command: "read-range",
                data: ReadRangeData { file, slices },
                warnings,
                meta: serde_json::json!({}),
            };
            output::print_json(&payload)?;
        }
        _ => {
            for warning in &warnings {
                eprintln!("warning: {warning}");
            }
            print_compact(&file, &slices, &no_number);
        }
    }

    Ok(0)
}

fn parse_range(raw: &str) -> Result<RangeSpec> {
    if let Some((point, context)) = raw.split_once('~') {
        let point = parse_point(point).with_context(|| format!("invalid range: {raw}"))?;
        let context = context
            .parse::<usize>()
            .with_context(|| format!("invalid range: {raw}"))?;
        return Ok(RangeSpec::Context(point, context));
    }

    if let Some((start, end)) = raw.split_once(':').or_else(|| raw.split_once('-')) {
        let start = parse_point(start).with_context(|| format!("invalid range: {raw}"))?;
        let end = parse_point(end).with_context(|| format!("invalid range: {raw}"))?;
        return Ok(RangeSpec::Range(start, end));
    }

    Ok(RangeSpec::One(
        parse_point(raw).with_context(|| format!("invalid range: {raw}"))?,
    ))
}

fn parse_point(raw: &str) -> Result<Point> {
    let raw = raw
        .strip_prefix('L')
        .or_else(|| raw.strip_prefix('l'))
        .unwrap_or(raw);
    match raw {
        "start" | "START" | "begin" | "BEGIN" => Ok(Point::Start),
        "end" | "END" | "finish" | "FINISH" => Ok(Point::End),
        _ => {
            let n = raw.parse::<usize>()?;
            if n == 0 {
                bail!("line numbers are 1-based");
            }
            Ok(Point::Line(n))
        }
    }
}

fn resolve_range(spec: &RangeSpec, request: &str, line_count: usize) -> Result<ResolvedRange> {
    let (raw_start, raw_end) = match spec {
        RangeSpec::One(point) => {
            let line = resolve_point(point, line_count);
            (line, line)
        }
        RangeSpec::Range(start, end) => {
            let start = resolve_point(start, line_count);
            let end = resolve_point(end, line_count);
            if start > end {
                bail!("invalid range {request}: start is after end");
            }
            (start, end)
        }
        RangeSpec::Context(point, context) => {
            let line = resolve_point(point, line_count);
            (line.saturating_sub(*context), line + context)
        }
    };

    let start_line = raw_start.clamp(1, line_count);
    let end_line = raw_end.clamp(1, line_count);
    Ok(ResolvedRange {
        request: request.to_string(),
        start_line,
        end_line: end_line.max(start_line),
        clipped: raw_start != start_line || raw_end != end_line,
    })
}

fn resolve_point(point: &Point, line_count: usize) -> usize {
    match point {
        Point::Start => 1,
        Point::End => line_count,
        Point::Line(n) => *n,
    }
}

fn read_text_file(path: &Path) -> Result<TextFile> {
    let raw = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let (encoding, text) = decode_text(&raw)?;
    let newline = detect_newline(&raw);
    let lines = split_lines(&text);

    Ok(TextFile {
        path: display_path(path),
        encoding,
        newline,
        lines,
    })
}

fn decode_text(raw: &[u8]) -> Result<(String, String)> {
    if raw.contains(&0) {
        bail!("binary files are not supported");
    }
    if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let text = std::str::from_utf8(&raw[3..]).context("invalid utf-8")?;
        return Ok(("utf-8-sig".into(), text.to_string()));
    }
    if raw.starts_with(&[0xFF, 0xFE]) || raw.starts_with(&[0xFE, 0xFF]) {
        bail!("utf-16 files are not supported");
    }
    if let Ok(text) = std::str::from_utf8(raw) {
        return Ok(("utf-8".into(), text.to_string()));
    }
    let (decoded, _, had_errors) = GBK.decode(raw);
    if !had_errors {
        return Ok(("gbk".into(), decoded.into_owned()));
    }
    Ok(("latin1".into(), raw.iter().map(|b| *b as char).collect()))
}

fn split_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                lines.push(text[start..i].to_string());
                i += 1;
                start = i;
            }
            b'\r' => {
                lines.push(text[start..i].to_string());
                i += if bytes.get(i + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
                start = i;
            }
            _ => i += 1,
        }
    }

    if start < text.len() {
        lines.push(text[start..].to_string());
    }

    lines
}

fn detect_newline(raw: &[u8]) -> String {
    if raw.is_empty() {
        return "none".into();
    }
    let has_crlf = raw.windows(2).any(|w| w == b"\r\n");
    let mut stripped = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if i + 1 < raw.len() && raw[i] == b'\r' && raw[i + 1] == b'\n' {
            i += 2;
        } else {
            stripped.push(raw[i]);
            i += 1;
        }
    }
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

fn print_compact(file: &ReadRangeFile, slices: &[ReadRangeSlice], no_number: &bool) {
    // let only_one_slice = slices.len() == 1;
    // println!(
    //     "{} \u{2502} {} {} \u{2502} {} lines \u{2502} 1-based",
    //     file.path, file.encoding, file.newline, file.line_count
    // );
    for (idx, slice) in slices.iter().enumerate() {
        if idx > 0 {
            println!();
        }
        println!(
            "@@ Range Chunk \u{2502} {}:{}-{} (from-args={}) @@",
            file.path, slice.start_line, slice.end_line, slice.request
        );
        for (offset, line) in slice.content.split('\n').enumerate() {
            // println!("{:>3} \u{2502} {}", slice.start_line + offset, line);
            if !no_number {
                println!("{:>3} \u{2502} {}", slice.start_line + offset, line);
            } else {
                println!("{}", line);
            }
        }
    }
}

fn display_path(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.strip_prefix(&cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
