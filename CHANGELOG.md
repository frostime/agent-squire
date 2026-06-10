# Changelog

All notable changes to `agent-squire` are documented in this file.

Format based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added

- **`compose` command** — renders agent context templates into UTF-8 output.
  - Supports `stdin`, `file`, `env`, and guarded `exec` sources with `${{...}}` interpolation.
  - Writes rendered bodies to `%TEMP%/agent-temp` by default, with `--stdout` for pipeline mode.
  - Includes multiline interpolation, no-arg command colon omission, modifiers, fallback policies, `--check`, `--list-sources`, and `--prompt`.
  - Drains large `exec` output into bounded temp spill artifacts with JSON metadata and render-wide `--total-timeout` semantics.

## [v0.6.0] — 2026-06-03

### Added

- **`md-backlinks` command** — finds Markdown files that link to one or more focus pages.
  - Reuses the `md-links` parser/resolver so backlinks are based on resolved Markdown links, wiki links, and code path refs rather than raw text search.
  - Supports `--from` corpus selection, `.gitignore`-aware directory scans, compact output, and JSON output.

### Changed

- **BREAKING**: `md-links` no longer accepts command-local `--workspace`; use global `--cwd <root>` before the subcommand instead.
- **BREAKING**: `md-links` JSON metadata now reports `meta.cwd` instead of `meta.workspace`.
- `md-links` and `md-backlinks` now share the same effective-CWD path-root model for agent-facing usage.
- Fixed repository clippy baseline so `cargo clippy --all-targets --all-features -- -D warnings` passes.

## [v0.5.1] — 2026-06-03

### Changed

- **`read-lines` command**: internally accepts common agent-generated slice variants `L10-L50` and `10:50` while keeping public help focused on the documented `10-50` form.

## [v0.5.0] — 2026-06-01

### Added

- **`md-links` command** (alias `mdlinks`) — extracts Markdown link references for graph-building.
  - Supports Markdown links/images, wiki links, inline code path refs, angle refs, and SiYuan block refs.
  - Resolves file targets against source files and a workspace, with existence checks and JSON output.

## [v0.4.1] — 2026-05-28

### Added

- **Interactive `patch-edit` mode** (`-i` / `--input`):
  - Opens `$EDITOR` / `$VISUAL` with a temporary patch file; falls back to terminal paste mode (type `.` on a lone line to finish).
  - Always runs a dry-run preview first.
  - Optionally shows a unified diff before applying.
  - Prompts for confirmation before writing.
- `--prompt` flag to print the patch format specification.

## [v0.4.0] — 2026-05-27

### Added

- **`imgweb` command** — a local web UI for composing multi-image prompts.
  - Binds `127.0.0.1` on a random port with token-based auth.
  - Upload, reorder, edit, and delete images.
  - Generates a structured prompt from the image list.
  - Stores uploaded images in a temp session directory; files persist after exit so `file://` references remain usable.
  - Options: `--no-open` (skip browser auto-open), `--max-mb` (request body limit, default 25 MB).
- New dependencies: `axum`, `tokio`, `tower-http`, `open`, `uuid`.

## [v0.3.0] — 2026-05-24

### Added

- **`read-lines` command** (alias `lines`) — reads specified 1-based line slices from a text file.
  - Slice syntax: `N`, `A-B`, `N~K` (context window), `start`, `end`.
  - Repeated `--slice` / `-s` for multiple ranges; request order preserved.
  - Supports UTF-8, UTF-8 BOM, GBK, Windows-1252 encoding fallback.
  - Compact and JSON output.
- Integration test: `tests/read_lines.rs`.

### Changed

- License changed from MIT to **GPL-3.0**.

## [v0.2.2] — 2026-05-22

### Changed

- **Performance refactor** of `tree` and `toc` builtins — internal rewrites for improved output speed and structure.

## [v0.2.1] — 2026-05-22

### Added

- **`now` command** — prints the current local date and time. JSON mode returns structured date/time/timezone fields.

### Changed

- CLI argument handling improvements and help text refinements.

## [v0.2.0] — 2026-05-22

### Added

- Top-level command names: `file-tree`, `file-info`, `md-toc`, `read-lines`, `patch-edit` (previous names retained as aliases).

### Changed

- CLI performance optimizations and dependency cleanup.
- `info` output aligned with the Python reference implementation.
- `patch-edit` sentinel and matching logic cleaned up.

### Removed

- `map` subcommand (external command mapping still available via config).
- `MANIFEST.json`.
- `docs/architecture.md`; `patch-edit-compatibility.md` moved into `src/builtins/patch_edit/`.

## [v0.1.0] — 2026-05-22

### Added

- Initial release.
- Three binaries: `squire`, `agent-squire`, `asq`.
- Built-in commands: `tree`, `info`, `toc`, `patch-edit`, `list`.
- External command mapping via `~/.config/agent-squire/config.toml` and `.agent-squire.toml`.
- Global options: `--cwd`, `--print` (compact / json / ndjson / text / raw), `--json`.
- Input sources: `@stdin`, `@file:path`, `@env:NAME`, `@@file:path` (literal escape).
- `patch-edit`:
  - SEARCH / REPLACE, CREATE, OVERWRITE block types.
  - 1-based line ranges (`L10-L25`, `L10-`, `-L25`).
  - Exact match → loose match fallback.
  - Same-file multi-patch two-phase matching.
  - Already-applied and ambiguous match detection.
  - Overlap detection before writing.
  - Atomic writes with permission preservation and newline-style preservation.
  - Encoding fallback: UTF-8, UTF-8 BOM, GBK, Windows-1252.
  - `--dry-run` and `--yes` safety flags.
- Tests: `tests/clap.rs`, `tests/patch_edit_compat.rs`.

---

[v0.6.0]: https://github.com/frostime/agent-squire/compare/v0.5.1...v0.6.0
[v0.5.1]: https://github.com/frostime/agent-squire/compare/v0.5.0...v0.5.1
[v0.5.0]: https://github.com/frostime/agent-squire/compare/v0.4.1...v0.5.0
[v0.4.1]: https://github.com/frostime/agent-squire/compare/v0.4.0...v0.4.1
[v0.4.0]: https://github.com/frostime/agent-squire/compare/v0.3.0...v0.4.0
[v0.3.0]: https://github.com/frostime/agent-squire/compare/v0.2.2...v0.3.0
[v0.2.2]: https://github.com/frostime/agent-squire/compare/v0.2.1...v0.2.2
[v0.2.1]: https://github.com/frostime/agent-squire/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/frostime/agent-squire/compare/v0.1.0...v0.2.0
[v0.1.0]: https://github.com/frostime/agent-squire/releases/tag/v0.1.0
