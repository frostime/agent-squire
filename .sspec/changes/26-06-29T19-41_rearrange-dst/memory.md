# Memory: rearrange-dst

**Updated**: 2026-06-29T20:11+08:00

## Git Baseline (Immutable)
<!-- Captured during `sspec change new` before any change files are written.
This section records the change starting point in git and MUST NOT be edited or refreshed later. -->

- Captured: before change file creation
- Repository: `H:/SrcCode/playground/agent-squire`
- Branch: `feat/rearange`
- HEAD: `2e90439764a3b6b5091dbb253612c55ee1e15956`
- Worktree: `dirty`
- Status Snapshot: raw `git status --short --branch` output

```text
## feat/rearange
A  SPEC.md
```

## State

Implementation complete; change is in REVIEW. `asq rearrange` now implements the DST state-transition DSL from `reference/DST-SPEC.md`; old v1 action DSL is removed.

## Key Files

- `.sspec/changes/26-06-29T19-41_rearrange-dst/reference/DST-SPEC.md` — source behavior spec for the accepted Arrange state-transition DSL.
- `.sspec/changes/26-06-29T19-41_rearrange-dst/spec.md` — behavior contract and implementation scope.
- `.sspec/changes/26-06-29T19-41_rearrange-dst/design.md` — target module architecture and runtime flow.
- `src/builtins/rearrange/ast.rs` — parsed DSL AST and file-state vocabulary.
- `src/builtins/rearrange/error.rs` — structured error codes and line-aware error type.
- `src/builtins/rearrange/parser.rs` — DST parser using line scanning plus payload parsing.
- `src/builtins/rearrange/path.rs` — path identity resolver and prefix conflict validation.
- `src/builtins/rearrange/plan.rs` — pre-state snapshot, semantic validation, material registry, materialization, apply.
- `src/builtins/rearrange/textio.rs` — text decode/render/write/delete with mkdir and encoding-safe writes.
- `src/builtins/rearrange/output.rs` — compact/json DST preview.
- `tests/rearrange.rs` — DST integration coverage.

## Knowledge

- [2026-06-29T19:41+08:00] [Decision] Keep the public command name `rearrange`; do not introduce `arrange` or alternate DSL spellings.
- [2026-06-29T19:41+08:00] [Decision] Old v1 DSL behavior is discarded; do not build compatibility mode for `move/copy/delete/rearrange` action scripts.
- [2026-06-29T19:41+08:00] [Constraint] Do not modify the accepted DSL in `reference/DST-SPEC.md` during implementation unless user explicitly reopens DSL design.
- [2026-06-29T19:41+08:00] [Decision] Parser may borrow patch-edit's line-role-tokenization idea, but should not share parser code or use regex extraction as the main parser because the entire rearrange document must be valid DSL.
- [2026-06-29T19:41+08:00] [Decision] Missing target parent directories may be created during apply.
- [2026-06-29T19:41+08:00] [Decision] `before` coverage accepts numeric EOF ranges when they resolve to the actual file end; prompt should recommend `A-end` as the clearer default.
- [2026-06-29T19:41+08:00] [Decision] Path prefix conflicts in the same spec should fail to avoid implicit directory state transitions.
- [2026-06-29T20:11+08:00] [Gotcha] `cargo test rearrange --quiet` filters integration tests by test name; full confidence came from `cargo test --quiet` plus sandbox CLI dry-run/yes checks.

## Milestones

- [2026-06-29T19:41+08:00] Created new `rearrange-dst` change and moved root `SPEC.md` to `reference/DST-SPEC.md`.
- [2026-06-29T20:11+08:00] Implemented DST rewrite for `asq rearrange`; `cargo test --quiet` (161 tests), clippy, fmt, and sandbox CLI checks passed.
