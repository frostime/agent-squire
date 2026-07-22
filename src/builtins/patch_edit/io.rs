use std::borrow::Cow;
use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use encoding_rs::{GBK, WINDOWS_1252};

#[derive(Debug, Clone, Copy)]
pub enum TextEncoding {
    Utf8,
    Utf8Sig,
    Gbk,
    Windows1252,
}

pub fn read_target_text_with_encoding(path: &Path) -> Result<(String, TextEncoding)> {
    let raw = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;

    if crate::shared::encoding::has_utf8_bom(&raw) {
        let text = String::from_utf8_lossy(&raw[3..]).into_owned();
        return Ok((text, TextEncoding::Utf8Sig));
    }

    if let Ok(text) = String::from_utf8(raw.clone()) {
        return Ok((text, TextEncoding::Utf8));
    }

    let (cow, _, had_errors) = GBK.decode(&raw);
    if !had_errors {
        return Ok((cow.into_owned(), TextEncoding::Gbk));
    }

    let (cow, _, had_errors) = WINDOWS_1252.decode(&raw);
    if !had_errors {
        return Ok((cow.into_owned(), TextEncoding::Windows1252));
    }

    Ok((
        String::from_utf8_lossy(&raw).into_owned(),
        TextEncoding::Utf8,
    ))
}

pub fn atomic_write_text(path: &Path, text: &str, encoding: TextEncoding) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;

    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temp file in {}", parent.display()))?;

    let bytes = encode_text(text, encoding);
    tmp.write_all(&bytes)?;

    if let Ok(meta) = fs::metadata(path) {
        let _ = tmp.as_file().set_permissions(meta.permissions());
    }

    tmp.persist(path)
        .map_err(|err| anyhow::anyhow!("failed to persist {}: {}", path.display(), err.error))?;

    Ok(())
}

fn encode_text(text: &str, encoding: TextEncoding) -> Vec<u8> {
    match encoding {
        TextEncoding::Utf8 => text.as_bytes().to_vec(),
        TextEncoding::Utf8Sig => {
            let mut out = vec![0xEF, 0xBB, 0xBF];
            out.extend_from_slice(text.as_bytes());
            out
        }
        TextEncoding::Gbk => {
            let (cow, _, had_errors) = GBK.encode(text);
            if had_errors {
                text.as_bytes().to_vec()
            } else {
                match cow {
                    Cow::Borrowed(bytes) => bytes.to_vec(),
                    Cow::Owned(bytes) => bytes,
                }
            }
        }
        TextEncoding::Windows1252 => {
            let (cow, _, had_errors) = WINDOWS_1252.encode(text);
            if had_errors {
                text.as_bytes().to_vec()
            } else {
                match cow {
                    Cow::Borrowed(bytes) => bytes.to_vec(),
                    Cow::Owned(bytes) => bytes,
                }
            }
        }
    }
}
