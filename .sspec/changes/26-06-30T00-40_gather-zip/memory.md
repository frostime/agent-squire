# Memory: gather-zip

**Updated**: 2026-06-30T01:00

## Git Baseline (Immutable)

- Captured: before change file creation
- Repository: `H:/SrcCode/playground/agent-squire`
- Branch: `feat/gather-zip`
- HEAD: `56812c592c5c5f5583e51c99c0fcc17e7a463478`
- Worktree: `clean`

```text
## feat/gather-zip
```

## State

Implementation complete. All 6 phases done. 45 lib + 23 integration tests passing. Clippy clean.
Ready for user review (sspec-review phase).

## Key Files

- `src/builtins/gather/zip.rs` — core zip assembly logic (570 lines): `assemble_zip()`, `collect_entries()`, `assemble_staging_dir()`, `create_zip_archive()`, `collect_warnings_and_confirm()`, manifest generation
- `src/builtins/gather/interactive.rs` — `InteractiveCommand::Zip` variant, `/zip` parsing, main loop wiring
- `src/builtins/gather/model.rs` — existing `LineRange` type reused for ranged file slicing
- `src/builtins/gather/sources.rs` — `expand_dir()`, `expand_glob()`, `render_tree()` reused
- `tests/gather.rs` — 4 new `/zip` integration tests appended at end

## Knowledge

- [2026-06-30T01:00] Decision: Draft B (Agent Package) chosen over Draft A (Mirror+Index) — zip uses `files/` + `artifacts/` + `manifest.json`, no manifest.md
- [2026-06-30T01:00] Decision: Ranged files (`file:path:10-20`) go to `artifacts/` as text snippets, not full files in `files/`
- [2026-06-30T01:00] Decision: External CLI for zip creation — `powershell Compress-Archive` on Windows, `zip -r` on Unix. Consistent with gather's existing fzf external dependency pattern.
- [2026-06-30T01:00] Decision: Binary files included but warned; >10MB files warned. Single merged confirmation prompt.
- [2026-06-30T01:00] Decision: Cross-volume rename fallback: `fs::rename` → on failure `fs::copy` + `fs::remove_file`. Handles Windows temp dir (C:) to project dir (H:) moves.
- [2026-06-30T01:00] Constraint: `/zip` only in interactive mode (`-i`). No `--zip` CLI flag (deferred).
- [2026-06-30T01:00] Gotcha: `parse_interactive_command` uses `trim_start()` not `trim()` for `/zip` argument parsing — `trim()` would eat the space before `/done` suffix.
- [2026-06-30T01:00] Rejected: `manifest.md` was in original Draft B design but removed — manifest.json covers all indexing needs and is machine-readable. Agent doesn't need human-readable index.

## Milestones

- [2026-06-30T00:40] Created change `26-06-30T00-40_gather-zip` on branch `feat/gather-zip`
- [2026-06-30T00:42] Clarify phase: converged on Draft B (Agent Package) design
- [2026-06-30T00:44] Design phase: spec.md + design.md filled, @align confirmed
- [2026-06-30T00:45] Plan phase: tasks.md with 6 phases, 24 tasks
- [2026-06-30T00:48] Phase 1 complete: data model + scaffolding, 6 tests pass
- [2026-06-30T01:00] Phases 2-6 complete: full implementation + 4 integration tests, 79 total tests pass, clippy clean
