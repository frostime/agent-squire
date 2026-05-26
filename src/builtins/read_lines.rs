use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Args;
use encoding_rs::GBK;
use serde::Serialize;

use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

#[derive(Args, Debug)]
#[command(
    long_about = "Read known 1-based line slices from one text file.\n\nUse this when an agent already knows the exact line numbers to inspect and needs a cross-platform replacement for `sed -n` / PowerShell line slicing. It does not search text, parse syntax, or find symbols; use `rg`, `md-toc`, or language tooling for discovery first.\n\nSlice syntax is 1-based and inclusive:\n  N        one line\n  A-B      range from A through B\n  N~K      K lines before and after N\n\nN/A/B may be a positive integer, `start`, or `end`. Repeat --slice to read multiple slices; request order and duplicates are preserved.",
    after_help = "Examples:\n  squire lines src/cli.rs -s 10\n  squire lines src/cli.rs -s 10-30\n  squire lines src/cli.rs -s 120~20\n  squire lines src/cli.rs -s start-80 -s 120-end\n  squire --print json lines src/cli.rs -s 10-30"
)]
pub struct ReadLinesArgs {
    #[arg(help = "Text file to read")]
    pub file: PathBuf,

    #[arg(
        short = 's',
        long = "slice",
        value_name = "SPEC",
        required = true,
        help = "Repeatable 1-based slice: N, A-B, or N~K; supports start/end",
        long_help = "Repeatable 1-based inclusive line slice selector. Forms: N, A-B, N~K. N/A/B may be a positive integer, start, or end. K must be a non-negative integer."
    )]
    pub slices: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ReadLinesData {
    file: ReadLinesFile,
    slices: Vec<ReadLinesSlice>,
}

#[derive(Debug, Serialize)]
struct ReadLinesFile {
    path: String,
    encoding: String,
    newline: String,
    line_count: usize,
}

#[derive(Debug, Serialize)]
struct ReadLinesSlice {
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
enum SliceSpec {
    One(Point),
    Range(Point, Point),
    Context(Point, usize),
}

#[derive(Debug)]
struct ResolvedSlice {
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

pub fn run(args: ReadLinesArgs, ctx: &CommandContext) -> Result<u8> {
    let specs = args
        .slices
        .iter()
        .map(|spec| parse_slice(spec))
        .collect::<Result<Vec<_>>>()?;

    let text = read_text_file(&args.file)?;
    if text.lines.is_empty() {
        bail!("{} has no lines", text.path);
    }

    let mut warnings = Vec::new();
    let resolved = specs
        .iter()
        .zip(args.slices.iter())
        .map(|(spec, request)| resolve_slice(spec, request, text.lines.len()))
        .collect::<Result<Vec<_>>>()?;

    let slices = resolved
        .iter()
        .map(|slice| {
            if slice.clipped {
                warnings.push(format!(
                    "slice {} clipped to {}-{}",
                    slice.request, slice.start_line, slice.end_line
                ));
            }
            ReadLinesSlice {
                request: slice.request.clone(),
                start_line: slice.start_line,
                end_line: slice.end_line,
                content: text.lines[(slice.start_line - 1)..slice.end_line].join("\n"),
            }
        })
        .collect::<Vec<_>>();

    let file = ReadLinesFile {
        path: text.path,
        encoding: text.encoding,
        newline: text.newline,
        line_count: text.lines.len(),
    };

    match ctx.print {
        PrintMode::Json => {
            let payload = Envelope {
                ok: true,
                command: "read-lines",
                data: ReadLinesData { file, slices },
                warnings,
                meta: serde_json::json!({}),
            };
            output::print_json(&payload)?;
        }
        _ => {
            for warning in &warnings {
                eprintln!("warning: {warning}");
            }
            print_compact(&file, &slices);
        }
    }

    Ok(0)
}

fn parse_slice(raw: &str) -> Result<SliceSpec> {
    if let Some((point, context)) = raw.split_once('~') {
        let point = parse_point(point).with_context(|| format!("invalid slice: {raw}"))?;
        let context = context
            .parse::<usize>()
            .with_context(|| format!("invalid slice: {raw}"))?;
        return Ok(SliceSpec::Context(point, context));
    }

    if let Some((start, end)) = raw.split_once('-') {
        let start = parse_point(start).with_context(|| format!("invalid slice: {raw}"))?;
        let end = parse_point(end).with_context(|| format!("invalid slice: {raw}"))?;
        return Ok(SliceSpec::Range(start, end));
    }

    Ok(SliceSpec::One(
        parse_point(raw).with_context(|| format!("invalid slice: {raw}"))?,
    ))
}

fn parse_point(raw: &str) -> Result<Point> {
    match raw {
        "start" => Ok(Point::Start),
        "end" => Ok(Point::End),
        _ => {
            let n = raw.parse::<usize>()?;
            if n == 0 {
                bail!("line numbers are 1-based");
            }
            Ok(Point::Line(n))
        }
    }
}

fn resolve_slice(spec: &SliceSpec, request: &str, line_count: usize) -> Result<ResolvedSlice> {
    let (raw_start, raw_end) = match spec {
        SliceSpec::One(point) => {
            let line = resolve_point(point, line_count);
            (line, line)
        }
        SliceSpec::Range(start, end) => {
            let start = resolve_point(start, line_count);
            let end = resolve_point(end, line_count);
            if start > end {
                bail!("invalid slice {request}: start is after end");
            }
            (start, end)
        }
        SliceSpec::Context(point, context) => {
            let line = resolve_point(point, line_count);
            (line.saturating_sub(*context), line + context)
        }
    };

    let start_line = raw_start.clamp(1, line_count);
    let end_line = raw_end.clamp(1, line_count);
    Ok(ResolvedSlice {
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
    let without_final_newline = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .or_else(|| text.strip_suffix('\r'))
        .unwrap_or(text);
    if without_final_newline.is_empty() {
        Vec::new()
    } else {
        without_final_newline
            .split_inclusive(['\n', '\r'])
            .map(|line| line.trim_end_matches(['\n', '\r']).to_string())
            .collect()
    }
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

fn print_compact(file: &ReadLinesFile, slices: &[ReadLinesSlice]) {
    println!(
        "{} | {} {} | {} lines | 1-based",
        file.path, file.encoding, file.newline, file.line_count
    );
    println!();
    for (idx, slice) in slices.iter().enumerate() {
        if idx > 0 {
            println!();
        }
        println!(
            "@@ {}-{} requested={}",
            slice.start_line, slice.end_line, slice.request
        );
        for (offset, line) in slice.content.split('\n').enumerate() {
            println!("{:>3} | {}", slice.start_line + offset, line);
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
