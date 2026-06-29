//! Text I/O for `rearrange`.
//!
//! Planner logic works with logical lines. This module owns decoding, newline
//! metadata, final-newline metadata, encoding-safe rendering, and atomic writes.

use std::borrow::Cow;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone)]
pub struct TextFile {
    pub lines: Vec<String>,
    pub encoding: Encoding,
    pub newline: Newline,
    pub trailing_newline: bool,
}

#[derive(Debug)]
pub enum TextIoError {
    Io(anyhow::Error),
    Encoding(String),
}

impl std::fmt::Display for TextIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err:#}"),
            Self::Encoding(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for TextIoError {}

pub fn read_file(path: &Path) -> std::result::Result<TextFile, TextIoError> {
    let raw = fs::read(path)
        .with_context(|| format!("failed to read {}", path.display()))
        .map_err(TextIoError::Io)?;
    let (text, encoding) = decode(&raw).map_err(TextIoError::Encoding)?;
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
    pub fn is_empty_file(&self) -> bool {
        self.lines.is_empty() && !self.trailing_newline
    }

    pub fn render_existing(&self, lines: &[String]) -> std::result::Result<Vec<u8>, String> {
        let mut text = lines.join(self.newline.as_str());
        if self.trailing_newline && !lines.is_empty() {
            text.push_str(self.newline.as_str());
        }
        encode(&text, self.encoding)
    }
}

pub fn render_created(lines: &[String]) -> Vec<u8> {
    let mut text = lines.join("\n");
    if !lines.is_empty() {
        text.push('\n');
    }
    text.into_bytes()
}

pub struct PreparedWrite {
    path: PathBuf,
    tmp: tempfile::NamedTempFile,
}

impl PreparedWrite {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn persist(self) -> Result<()> {
        self.tmp.persist(&self.path).map_err(|err| {
            anyhow::anyhow!("failed to persist {}: {}", self.path.display(), err.error)
        })?;
        Ok(())
    }
}

pub fn prepare_write(path: &Path, bytes: &[u8]) -> Result<PreparedWrite> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;

    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temp file in {}", parent.display()))?;
    tmp.write_all(bytes)?;
    if let Ok(meta) = fs::metadata(path) {
        let _ = tmp.as_file().set_permissions(meta.permissions());
    }
    Ok(PreparedWrite {
        path: path.to_path_buf(),
        tmp,
    })
}

pub fn delete_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to delete {}", path.display())),
    }
}

fn decode(raw: &[u8]) -> std::result::Result<(String, Encoding), String> {
    if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let text = std::str::from_utf8(&raw[3..])
            .map_err(|err| format!("invalid utf-8 BOM text: {err}"))?;
        return Ok((text.to_string(), Encoding::Utf8Bom));
    }
    if let Ok(text) = String::from_utf8(raw.to_vec()) {
        return Ok((text, Encoding::Utf8));
    }
    let (text, _, had_errors) = GBK.decode(raw);
    if !had_errors {
        return Ok((text.into_owned(), Encoding::Gbk));
    }
    let (text, _, had_errors) = WINDOWS_1252.decode(raw);
    if !had_errors {
        return Ok((text.into_owned(), Encoding::Windows1252));
    }
    Err("unsupported text encoding; refusing lossy decode".into())
}

fn encode(text: &str, encoding: Encoding) -> std::result::Result<Vec<u8>, String> {
    match encoding {
        Encoding::Utf8 => Ok(text.as_bytes().to_vec()),
        Encoding::Utf8Bom => {
            let mut out = vec![0xEF, 0xBB, 0xBF];
            out.extend_from_slice(text.as_bytes());
            Ok(out)
        }
        Encoding::Gbk => encode_checked(text, Encoding::Gbk),
        Encoding::Windows1252 => encode_checked(text, Encoding::Windows1252),
    }
}

fn encode_checked(text: &str, encoding: Encoding) -> std::result::Result<Vec<u8>, String> {
    let (cow, _, had_errors) = match encoding {
        Encoding::Gbk => GBK.encode(text),
        Encoding::Windows1252 => WINDOWS_1252.encode(text),
        _ => unreachable!("checked encodings only"),
    };
    if had_errors {
        return Err(format!("text cannot be encoded as {encoding:?}"));
    }
    Ok(match cow {
        Cow::Borrowed(bytes) => bytes.to_vec(),
        Cow::Owned(bytes) => bytes,
    })
}

fn detect_newline(text: &str) -> Newline {
    if text.as_bytes().windows(2).any(|w| w == b"\r\n") {
        Newline::Crlf
    } else {
        Newline::Lf
    }
}

fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> = text
        .split('\n')
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gbk_render_fails_on_unencodable_text() {
        let file = TextFile {
            lines: vec!["原文".into()],
            encoding: Encoding::Gbk,
            newline: Newline::Lf,
            trailing_newline: true,
        };
        let err = file.render_existing(&["😀".into()]).unwrap_err();
        assert!(err.contains("Gbk"));
    }
}
