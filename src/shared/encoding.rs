//! Encoding-adjacent primitives shared by builtins that process text files.
//!
//! See `.sspec/spec-docs/shared-encoding.md` for the policy matrix each
//! builtin follows, why the high-level decode entries remain per-builtin, and
//! what this module intentionally does NOT consolidate.

/// Detected byte-order-marker kind, or absence thereof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bom {
    /// No BOM present.
    None,
    /// UTF-8 with BOM (0xEF 0xBB 0xBF).
    Utf8Sig,
    /// UTF-16 little-endian (0xFF 0xFE).
    Utf16Le,
    /// UTF-16 big-endian (0xFE 0xFF).
    Utf16Be,
    /// UTF-32 little-endian (0xFF 0xFE 0x00 0x00).
    Utf32Le,
    /// UTF-32 big-endian (0x00 0x00 0xFE 0xFF).
    Utf32Be,
}

impl Bom {
    /// Human-readable label matching `file-info`/`read-range` output (e.g.
    /// `"utf-8-sig"`, `"utf-16-le"`, `"none"`).
    pub fn label(self) -> &'static str {
        match self {
            Bom::None => "none",
            Bom::Utf8Sig => "utf-8-sig",
            Bom::Utf16Le => "utf-16-le",
            Bom::Utf16Be => "utf-16-be",
            Bom::Utf32Le => "utf-32-le",
            Bom::Utf32Be => "utf-32-be",
        }
    }
}

/// Detect a BOM at the start of `raw`. Longer BOMs are checked first because
/// UTF-32-LE starts with the same two bytes as UTF-16-LE; the longest match
/// wins so the kind is unambiguous.
pub fn detect_bom(raw: &[u8]) -> Bom {
    if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        Bom::Utf8Sig
    } else if raw.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        Bom::Utf32Le
    } else if raw.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        Bom::Utf32Be
    } else if raw.starts_with(&[0xFF, 0xFE]) {
        Bom::Utf16Le
    } else if raw.starts_with(&[0xFE, 0xFF]) {
        Bom::Utf16Be
    } else {
        Bom::None
    }
}

/// True when `raw` begins with the UTF-8 BOM. Shortcut for builtins that only
/// recognize the UTF-8 BOM (`compose`, `patch-edit`, `rearrange`).
pub fn has_utf8_bom(raw: &[u8]) -> bool {
    raw.starts_with(&[0xEF, 0xBB, 0xBF])
}

/// Logical newline classification of *decoded text* (analysis on characters, not
/// raw bytes, so UTF-16 CRLF classifies the same as UTF-8 CRLF).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Newline {
    None,
    Lf,
    Crlf,
    Cr,
    Mixed,
}

impl Newline {
    pub fn label(self) -> &'static str {
        match self {
            Newline::None => "none",
            Newline::Lf => "lf",
            Newline::Crlf => "crlf",
            Newline::Cr => "cr",
            Newline::Mixed => "mixed",
        }
    }
}

/// Detect the newline style of decoded text. Treats CRLF as a single separator
/// before counting remaining LF/CR occurrences. Mirrors the previously
/// duplicated `file-info::detect_newline_text` / `read-range::detect_newline`.
pub fn detect_newline_text(text: &str) -> Newline {
    if text.is_empty() {
        return Newline::None;
    }
    let raw = text.as_bytes();
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
        (true, false, false) => Newline::Crlf,
        (false, true, false) => Newline::Lf,
        (false, false, true) => Newline::Cr,
        (false, false, false) => Newline::None,
        _ => Newline::Mixed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_bom_recognizes_all_kinds() {
        assert_eq!(detect_bom(&[]), Bom::None);
        assert_eq!(detect_bom(&[0xEF, 0xBB, 0xBF, b'x']), Bom::Utf8Sig);
        assert_eq!(detect_bom(&[0xFF, 0xFE, 0, 0, b'x']), Bom::Utf32Le);
        // UTF-16-LE must be distinguished from UTF-32-LE by longest-prefix rule:
        assert_eq!(detect_bom(&[0xFF, 0xFE, b'x']), Bom::Utf16Le);
        assert_eq!(detect_bom(&[0x00, 0x00, 0xFE, 0xFF]), Bom::Utf32Be);
        assert_eq!(detect_bom(&[0xFE, 0xFF, b'x']), Bom::Utf16Be);
        assert_eq!(detect_bom(b"x"), Bom::None);
    }

    #[test]
    fn label_strings_match_legacy_output() {
        assert_eq!(Bom::Utf8Sig.label(), "utf-8-sig");
        assert_eq!(Bom::None.label(), "none");
        assert_eq!(Bom::Utf16Le.label(), "utf-16-le");
        assert_eq!(Bom::Utf16Be.label(), "utf-16-be");
        assert_eq!(Bom::Utf32Le.label(), "utf-32-le");
        assert_eq!(Bom::Utf32Be.label(), "utf-32-be");
    }

    #[test]
    fn newline_detection_matches_legacy() {
        assert_eq!(detect_newline_text(""), Newline::None);
        assert_eq!(detect_newline_text("a\nb"), Newline::Lf);
        assert_eq!(detect_newline_text("a\r\nb"), Newline::Crlf);
        assert_eq!(detect_newline_text("a\rb"), Newline::Cr);
        assert_eq!(detect_newline_text("a\nb\r\n"), Newline::Mixed);
        assert_eq!(detect_newline_text("abc"), Newline::None);
    }

    #[test]
    fn has_utf8_bom_shortcut() {
        assert!(has_utf8_bom(&[0xEF, 0xBB, 0xBF, b'a']));
        assert!(!has_utf8_bom(&[0xEF, 0xBB]));
    }

    #[test]
    fn newline_label_strings() {
        assert_eq!(Newline::Lf.label(), "lf");
        assert_eq!(Newline::Crlf.label(), "crlf");
        assert_eq!(Newline::Cr.label(), "cr");
        assert_eq!(Newline::Mixed.label(), "mixed");
        assert_eq!(Newline::None.label(), "none");
    }
}
