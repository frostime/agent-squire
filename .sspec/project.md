# Project Context

<!-- This file is the stable identity layer for agents working on this project.
Read it first every session. Update Conventions + Notes via @memory. -->

**Name**: agent-squire
**Description**: A local CLI toolbox for humans and agents.
**Repo**: https://github.com/frostime/agent-squire

## Tech Stack
- Rust 2024 edition, clap 4 (derive), serde/toml, axum+tokio, encoding_rs, tempfile

## Key Paths
<!-- MUST keep ≤10 entries. Most important directories/files for quick navigation.
Agent uses this to orient in the codebase. -->

| Path | Purpose |
|------|---------|
| `src/cli.rs` | CLI entry, subcommand definitions, clap derive structs |
| `src/builtins/` | All built-in commands (each subdir = one vertical command module) |
| `src/shared/file_sources.rs` | Shared file/dir/glob source resolver + `SourcePolicy` |
| `src/shared/markdown.rs` | Shared fenced-code-block prose-line iterator |
| `src/builtins/patch_edit/` | Core command patch-edit; includes compatibility doc |
| `src/runtime/input.rs` | Input source abstraction (`@stdin`, `@file:path`, `@env:NAME`) |
| `src/runtime/output.rs` | Output mode (`PrintMode`, `Envelope<T>` JSON envelope) |
| `src/shared/encoding.rs` | Shared BOM detection + decoded-text newline classification |
| `src/shared/path.rs` | Path display helpers (slash-normalize, relative-to-base) |
| `src/external.rs` | External command mapping (TOML config parse + execution) |
| `.agent-squire.example.toml` | External command mapping config example |
| `CHANGELOG.md` | Version history, currently at v0.5.1 |

## Conventions
<!-- MUST be one-liners. Coding rules that apply across ALL work in this project.
If a convention needs multi-paragraph explanation → write a spec-doc.
Examples: "snake_case for Python, camelCase for JS", "All API routes: /api/v1/*",
"Never commit .env files", "Prefer composition over inheritance" -->

- `snake_case` for Rust modules/files; CLI command names use `kebab-case`
- Each builtin command = `src/builtins/<name>/mod.rs` vertical module
- CLI names: `file-tree` / `file-info` / `md-toc` / `read-range` / `patch-edit`; old names retained as aliases
- Output unified via `runtime::output::Envelope<T>` JSON envelope
- Agent-facing CLI output should be understandable and token-efficient; avoid repeating equivalent fields in compact output
- Input unified via `@stdin` / `@file:path` / `@env:NAME` source syntax
- Tests: unit tests in-module, integration tests in `tests/`
- Changelog format: Keep a Changelog; version in `Cargo.toml`
- GitHub `origin` is the primary repository; Codeberg `codeberg` is the secondary mirror
- Release tags should exist on both GitHub and Codeberg

## Spec-Docs Index
<!-- Quick reference to spec-docs in `.sspec/spec-docs/`.
Spec-docs capture knowledge that code alone cannot adequately convey:
  A) In code, but scattered or hard to reconstruct (cross-module architecture, UX requirements, design norms, trade-offs)
  B) Outside code entirely (platform rules, API quirks, business constraints, deployment assumptions)
NOT a restating of code behavior — if readable from code+comments, it doesn't belong here.
MUST keep entries in sync with actual spec-doc files.
Format: `- [name](spec-docs/<file>) — one-line summary` -->

- [Compose Template Engine](spec-docs/compose-template-engine.md) — parse/compile/render phases, source/modifier contracts, exec spill artifacts, and JSON output schema.
- [Source Resolver](spec-docs/builtin-source-resolver.md) — `SourcePolicy` axis table, caller mapping (toc/md-links/md-backlinks/file-info), `gitignore off/respect` equivalence, and known behavior deltas.
- [Shared Encoding Primitives](spec-docs/shared-encoding.md) — extracted BOM/newline helpers and the policy matrix that keeps each high-level decode per-builtin.

## Notes
<!-- Project-level memory. Append-only log of learnings, gotchas, preferences.
Agent appends here during @memory when a discovery is project-wide (not change-specific).
Format each entry as: `- YYYY-MM-DD: <learning>`
Prune entries that become outdated or graduate to Conventions/spec-docs. -->
