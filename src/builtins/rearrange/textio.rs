//! Text I/O for `rearrange`, the single home of newline and encoding knowledge.
//!
//! The planner never sees line terminators: [`read_file`] hands it logical
//! lines (terminator stripped) and [`TextFile`] remembers how to reassemble
//! them. [`write_file`] restores the original newline style, trailing-newline
//! presence, and byte encoding. Keeping this knowledge in one module is what
//! lets the planner reason purely about `Vec<String>`.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use encoding_rs::{GBK, WINDOWS_1252};

#[derive(Debug, Clone, Copy)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Gbk,
    Windows1252,
}

#[derive(Debug, Clone, Copy)]
pub enum Newline {
    Lf,
    Crlf,
}

impl Newline {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
        }
    }
}

/// A file decoded into logical lines plus the metadata needed to write it back
/// byte-for-byte compatible (encoding, newline style, trailing newline).
pub struct TextFile {
    pub lines: Vec<String>,
    pub encoding: Encoding,
    pub newline: Newline,
    pub trailing_newline: bool,
}

pub fn read_file(path: &Path) -> Result<TextFile> {
    let raw = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let (text, encoding) = decode(&raw);
    let newline = detect_newline(&text);
    let trailing_newline = text.ends_with('\n');
    let lines = split_lines(&text);
    Ok(TextFile {
        lines,
        encoding,
        newline,
        trailing_newline,
    })
}

impl TextFile {
    /// Reassemble logical lines into bytes using the original file's style.
    pub fn render(&self, lines: &[String]) -> Vec<u8> {
        let mut text = lines.join(self.newline.as_str());
        if self.trailing_newline && !lines.is_empty() {
            text.push_str(self.newline.as_str());
        }
        encode(&text, self.encoding)
    }
}

/// Atomically replace `path` with `bytes` via a same-directory temp file.
pub fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temp file in {}", parent.display()))?;
    tmp.write_all(bytes)?;
    if let Ok(meta) = fs::metadata(path) {
        let _ = tmp.as_file().set_permissions(meta.permissions());
    }
    tmp.persist(path)
        .map_err(|err| anyhow::anyhow!("failed to persist {}: {}", path.display(), err.error))?;
    Ok(())
}

fn decode(raw: &[u8]) -> (String, Encoding) {
    if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return (
            String::from_utf8_lossy(&raw[3..]).into_owned(),
            Encoding::Utf8Bom,
        );
    }
    if let Ok(text) = String::from_utf8(raw.to_vec()) {
        return (text, Encoding::Utf8);
    }
    let (text, _, had_errors) = GBK.decode(raw);
    if !had_errors {
        return (text.into_owned(), Encoding::Gbk);
    }
    let (text, _, had_errors) = WINDOWS_1252.decode(raw);
    if !had_errors {
        return (text.into_owned(), Encoding::Windows1252);
    }
    (String::from_utf8_lossy(raw).into_owned(), Encoding::Utf8)
}

fn encode(text: &str, encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Utf8Bom => {
            let mut out = vec![0xEF, 0xBB, 0xBF];
            out.extend_from_slice(text.as_bytes());
            out
        }
        Encoding::Gbk => GBK.encode(text).0.into_owned(),
        Encoding::Windows1252 => WINDOWS_1252.encode(text).0.into_owned(),
    }
}

/// SPEC: CRLF if any `\r\n` is present, else LF. Mixed input normalizes to the
/// dominant CRLF on write; v1 does not preserve mixed styles.
fn detect_newline(text: &str) -> Newline {
    if text.as_bytes().windows(2).any(|w| w == b"\r\n") {
        Newline::Crlf
    } else {
        Newline::Lf
    }
}

/// SPEC: split on `\n` and `\r\n` alike, treating CRLF as one separator so no
/// ghost blank lines appear. The terminator is stripped from each logical line.
fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> = text
        .split('\n')
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();
    // A trailing newline yields a final empty element; it is metadata, not a line.
    if text.ends_with('\n') {
        lines.pop();
    }
    lines
}
