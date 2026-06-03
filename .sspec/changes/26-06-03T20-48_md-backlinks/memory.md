# Memory: md-backlinks

**Updated**: 2026-06-03T21:53+08:00

## Git Baseline (Immutable)
<!-- Captured during `sspec change new` before any change files are written.
This section records the change starting point in git and MUST NOT be edited or refreshed later. -->

- Captured: before change file creation
- Repository: `H:/SrcCode/playground/agent-squire`
- Branch: `main`
- HEAD: `8f1a7247afb3e3065042d1426c50d70671470a2d`
- Worktree: `clean`
- Status Snapshot: raw `git status --short --branch` output

```text
## main...origin/main
```

## State

Feature implementation plus revision 001 are complete and accepted for merge. `cargo test` passes; clippy was run but fails on unrelated pre-existing baseline lints that the user requested handling in a separate branch.

## Key Files

- `.sspec/changes/26-06-03T20-48_md-backlinks/spec.md` — user-facing prediction contract for the new backlink command.
- `.sspec/changes/26-06-03T20-48_md-backlinks/design.md` — CLI contract, corpus policy, data schema, and TDD behavior matrix.
- `src/builtins/md_links/` — existing parser/resolver code that backlinks must reuse.
- `tests/md_links.rs` — compatibility tests that must remain unchanged in behavior.

## Knowledge

- [2026-06-03T20:48+08:00] [Decision] Add a separate `md-backlinks` command instead of overloading `md-links`; positional arguments mean focus pages in both mental models, while backlink corpus is specified by `--from`.
- [2026-06-03T20:48+08:00] [Decision] Backlinks are computed as a reverse view of resolved forward file-link edges: scan corpus files, parse links, resolve targets, then group edges whose target equals a focus page.
- [2026-06-03T20:48+08:00] [Constraint] Implementation must be test-driven: define integration tests for backlink behavior before/with code changes, and preserve all existing `tests/md_links.rs` expectations.
- [2026-06-03T20:48+08:00] [Rejected] Do not implement backlink discovery as `rg filename` followed by filtering; raw text search misses wiki/relative/fragment-normalized links and produces false positives from non-link text.
- [2026-06-03T20:48+08:00] [Rejected] Do not shell out to external `rg`; correctness-first implementation should use internal parser/resolver and Rust file discovery.
- [2026-06-03T20:48+08:00] [Insight] `file-tree` already uses `ignore::WalkBuilder` with `.gitignore` plus built-in skip names; backlink corpus discovery should align with that policy where practical.
- [2026-06-03T21:05+08:00] [Gotcha] `cargo clippy --all-targets --all-features -- -D warnings` currently fails on unrelated pre-existing lints in `src/builtins/imgweb/mod.rs`, `src/builtins/info/mod.rs`, `src/builtins/patch_edit/*`, `src/builtins/tree/mod.rs`, and `src/external.rs`; after fixing the new `md_backlinks` redundant-closure warning, no clippy error points at newly added backlink code.
- [2026-06-03T21:09+08:00] [Decision] User chose not to fix unrelated clippy baseline errors in `wip/md-backlinks`; a local ignored `.sspec/tmp` backlog file was written for later separate-branch work.
- [2026-06-03T21:29+08:00] [Decision] Revision 001 removes `md-backlinks --workspace`; the new backlinks command uses effective CWD as the single path root for focus pages, `--from` corpus, and link target resolution. Existing `md-links --workspace` remains unchanged for compatibility.
- [2026-06-03T21:53+08:00] [Decision] Duplicate focus page arguments are deduplicated by normalized path, preserving the first occurrence.

## Milestones

- [2026-06-03T20:48+08:00] Created change `26-06-03T20-48_md-backlinks` and drafted spec/design for user alignment; no production code changed.
- [2026-06-03T20:58+08:00] User requested continuing in WIP branch mode; planned TDD implementation tasks.
- [2026-06-03T21:05+08:00] Implemented `md-backlinks`, added integration tests, ran `cargo test` successfully; clippy gate remains blocked by unrelated pre-existing lint errors.
- [2026-06-03T21:06+08:00] Checkpoint commit recorded the md-backlinks implementation on `wip/md-backlinks`.
- [2026-06-03T21:09+08:00] Recorded unrelated clippy baseline backlog as local ignored tmp and moved change to REVIEW.
- [2026-06-03T21:29+08:00] Implemented revision 001, added `--cwd` semantics tests, removed `md-backlinks --workspace`, and returned change to REVIEW.
- [2026-06-03T21:53+08:00] Added duplicate-focus regression fix and marked change DONE for squash merge to main.
