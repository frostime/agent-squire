# Changelog

All notable changes to `agent-squire` are documented in this file.

Format based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added

- `data-toc` command (alias `datatoc`) to preview JSON/YAML/JSONL structure with bounded scans, array index compression, dynamic key compression, JSONL record groups, optional redacted examples, and an agent-facing `--prompt` guide.

## [v0.9.0] — 2026-06-25

### Added

- `img` command to save the current clipboard image as a persistent PNG path, with `--web` for the image prompt web UI.

### Changed

- `imgweb` remains executable for compatibility but is hidden from primary command discovery in favor of `img --web`.

## [v0.8.7] — 2026-06-18

### Added

- `tmp` command (alias `temp`) to create temporary files/directories under `<SYSTEM_TEMP_DIR>/asq-temp` or a custom `--root`. Supports timestamp prefix, type inference, and `--open`.

## [v0.8.6] — 2026-06-16

### Added

- `file-tree --detail`: directories with entries omitted from tree output now show `(N omitted items)`, covering depth limits, ignore filters, and built-in skip rules.

## [v0.8.5] — 2026-06-16

### Added

- `read-range` now supports UTF-16 LE/BE files when a BOM identifies endianness.
- `file-info` now reports newline style and line counts for UTF-16 BOM text files.

### Fixed

- `read-range` no longer inserts ghost blank lines or shifts line numbers for CRLF files.
- `file-info` no longer misclassifies UTF-16 CRLF files as `mixed` newline style.
- UTF-32 BOM files are no longer misclassified as UTF-16; `read-range` rejects them explicitly and `file-info` reports the unsupported UTF-32 BOM.
- `gather dir:...`, `tree:...`, and interactive candidates now respect the workspace `.gitignore` from the current working directory, including non-Git temp workspaces.

## [v0.8.4] — 2026-06-15

### Added

- **`patch-edit` `--smart-indent` flag** — migrates a SEARCH/REPLACE block between indentation levels instead of rejecting a literal indent mismatch.
  - Detects the base indent of SEARCH and the matched target window, then rewrites REPLACE from `indent_from` to `indent_to`.
  - Supports both indent increase and decrease; preserves relative indentation within the block.
  - Blank/whitespace-only lines are ignored for base-indent calculation.
  - Multiple candidate locations remain `search_indent_ambiguous`; a REPLACE line that cannot be migrated returns `replace_indent_incompatible`.
  - Second `--smart-indent` run detects already-applied state (`already_applied`).
- Structured indent metadata in results: `indent_from` and `indent_to` replace the old single `indent_delta`.
- Options-based Rust API: `apply_patches_with_options` / `apply_parsed_patches_with_options` with `PatchApplyOptions`; old 3-arg `apply_patches` still compiles.

### Changed

- `cargo fmt` applied project-wide, including pre-existing `src/builtins/read_range.rs` formatting.

## [v0.8.3] — 2026-06-14

### Added

- GitHub Actions release workflow (`.github/workflows/release.yml`).

### Changed

- `read-range` / `read-lines`: parameter optimization for agent-facing slice alias handling.

## [v0.8.1] — 2026-06-13

### Changed

- `read-lines` / `read-range`: new `head:N`, `tail:N`, and no-range (whole-file) modes; delimiter character change for slice syntax.
- Extended `read_lines` test coverage for new slice variants.

## [v0.8.0] — 2026-06-13

### Added

- **`gather` command** — assembles files, line ranges, directory/glob file groups, trees, and command output into one fenced prompt body.
  - Supports positional and named `file`, `dir`, `tree`, `glob`, and `cmd` sources.
  - Writes to `%TEMP%/agent-temp` by default with `asq-gather-*` names, with `--stdout` and `--output` modes.
  - Includes interactive fzf selector lines with editable `edit>` confirmation, `/help`/`/list`/`/done`/`/exit`/`/all` controls, and `--no-gitignore` for ignored-file selection.
  - Uses parent-qualified nested fences (`DIR-FILE-*`, `GLOB-FILE-*`) for expanded grouped files.
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

[v0.8.5]: https://github.com/frostime/agent-squire/compare/v0.8.4...v0.8.5
[v0.8.4]: https://github.com/frostime/agent-squire/compare/v0.8.3...v0.8.4
[v0.8.3]: https://github.com/frostime/agent-squire/compare/v0.8.1...v0.8.3
[v0.8.1]: https://github.com/frostime/agent-squire/compare/v0.8.0...v0.8.1
[v0.8.0]: https://github.com/frostime/agent-squire/compare/v0.6.0...v0.8.0
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
