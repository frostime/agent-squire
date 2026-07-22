# Runtime Encoding Primitives

Track knowledge that `src/runtime/encoding.rs` alone cannot adequately convey.

## Purpose

BOM detection and decoded-text newline classification are byte-level helpers
shared across `file-info`, `read-range`, `compose`, `patch-edit`, and
`rearrange`. Before this module each duplicated the same three or five
`raw.starts_with(...)` BOM probes and the same CRLF/LF/CR/mixed classification
loop. This module owns those primitives only.

## What is consolidated

- `Bom` enum + `detect_bom(&[u8])`: long-prefix-first BOM detection, unambiguously
  distinguishing UTF-32-LE (which shares its two-byte prefix with UTF-16-LE) by
  checking the 4-byte UTF-32 BOMs before the 2-byte UTF-16 ones.
- `Bom::label()`: canonical label strings (`"utf-8-sig"`, `"utf-16-le"`,
  `"utf-32-be"`, `"none"`) reused by `file-info` and `read-range`.
- `has_utf8_bom(&[u8])`: shortcut for the many builtins that only recognize the
  UTF-8 BOM (`compose`, `patch-edit`, `rearrange`).
- `Newline` enum + `detect_newline_text(&str)` + `Newline::label()`: CRLF-aware
  classification of *decoded text* (not raw bytes) so UTF-16 CRLF and UTF-8 CRLF
  classify identically. Replaces the inline copy in `file-info` and `read-range`.

## What is intentionally NOT consolidated

Each builtin still owns its high-level text decode entry because the failure
policy differs materially between commands:

| Builtin | Binary | UTF-16 | UTF-32 BOM | Lossy fallback |
|---|---|---|---|---|
| `file-info` | tolerate (binary detect, sample) | recognized | recognized (label only) | latin1 (lossy) |
| `read-range` | refuse (`bail!`) | require BOM | refuse (`bail!`) | latin1 (lossy) |
| `compose` | refuse (NUL/refuse) | unsupported | unsupported | utf8 lossy |
| `patch-edit` | tolerate (utf8-lossy) | unsupported | unsupported | utf8 lossy |
| `rearrange` | tolerate | unsupported | unsupported | refuse (strict) |

Encoding the four-axis policy (`BinaryMode`, `Utf16Mode`, `Utf32BomMode`,
`LossyMode`) into a single generic `decode(raw, policy)` was rejected: it adds a
verbatim policy enum that each caller must populate, moves complexity from the
caller to the policy contract, and every cell above renders the shared entry
*narrower*, not unified. The unambiguous duplicates (BOM bytes, newline
algorithm) are extracted; the policy-laden decode cascade stays per-builtin.

## Future extension trigger

Should a new builtin reuse the same decode policy as an existing one (e.g.
another "read text safely and refuse binary" path), revisit whether the policy
matrix is worth lifting; until then the per-builtin decode entries remain.