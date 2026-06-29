---
change: "rearrange-dst"
updated: "2026-06-29T22:42+08:00"
---

# Tasks

## Legend
`[ ]` Todo | `[x]` Done

## Tasks

### Phase 1: Scaffold new rearrange architecture ✅
- [x] Move root `SPEC.md` into `reference/DST-SPEC.md` for this change.
- [x] Update change `spec.md` / `design.md` / `tasks.md` for the new DST implementation.
- [x] Rewrite `src/builtins/rearrange/mod.rs` to route the new parser/plan/output flow.
- [x] Add `src/builtins/rearrange/error.rs` and `src/builtins/rearrange/ast.rs`.

**Verification**:
- Agent: `git status --short` shows no root `SPEC.md`; new reference file exists.
- Agent: `cargo check --quiet` passed.

### Phase 2: Parser and path identity ✅
- [x] Rewrite `src/builtins/rearrange/parser.rs` for the DST DSL.
- [x] Add `src/builtins/rearrange/path.rs` for path identity and prefix conflict validation.
- [x] Add parser/path unit tests for identifiers, ranges, block order, invalid lines, duplicate slugs, duplicate paths, and path escape.

**Verification**:
- Agent: parser/path tests covered by `cargo test --quiet` passing.

### Phase 3: Snapshot planner and materialization ✅
- [x] Rewrite `src/builtins/rearrange/plan.rs` to validate shares, before coverage, explicit gaps, after provenance, state transitions, and materialized target outputs.
- [x] Update `src/builtins/rearrange/textio.rs` for create/delete/empty outputs, mkdir parents, and encoding-safe writes.
- [x] Add unit tests for hidden gaps, explicit gaps, named-vs-anonymous references, cross-file extraction, missing/empty transitions, and encoding failure path.

**Verification**:
- Agent: planner/text behavior covered by `cargo test --quiet` passing.

### Phase 4: Output, prompt, integration tests ✅
- [x] Rewrite `src/builtins/rearrange/output.rs` for compact/json DST preview.
- [x] Rewrite `src/builtins/rearrange/prompt.md` for the accepted DSL; recommend `A-end` while allowing numeric EOF guards.
- [x] Replace `tests/rearrange.rs` v1 tests with DST behavior tests.

**Verification**:
- Agent: `cargo test rearrange --quiet` passed.
- Agent: sandbox CLI dry-run showed no write; `--yes` applied expected multi-file result.

**User Check**:
1. BC-1/BC-2: `asq rearrange --prompt` shows only DST DSL, no old `move/copy/delete/rearrange` action DSL.
2. BC-5: dry-run preview lists shares, targets, before/after states, exports/effects, and no default full-file diff.

### Phase 5: Quality gate ✅
- [x] Run full test/format/lint gate.
- [x] Update `memory.md` with implementation outcome and gotchas.

**Verification**:
- Agent: `cargo test --quiet` passed: 161 tests.
- Agent: `cargo clippy --all-targets --all-features -- -D warnings` passed.
- Agent: `cargo fmt --check` passed.

### Feedback Tasks ✅
- [x] Fix external review F1: fail on invalid UTF-8 BOM text instead of lossy rewrite.
- [x] Fix external review F2: reject ambiguous unspaced `arrange slug=path` instead of treating it as a literal path or slugged arrange.
- [x] Fix external review F3: reject empty sequence items and trailing commas.
- [x] Fix external review F4: prepare all writes before persisting and report partial-apply risk on later failure.
- [x] Fix external review F5: update `rearrange --help` summary.
- [x] Delete stale old-v1 `src/builtins/rearrange/model.rs`.
- [x] Add regression tests for review findings and high-value invariants.
- [x] Fix follow-up parser ambiguity: unslugged `arrange` targets containing `=` now fail; use `arrange <slug> = <file>` to target paths containing `=`.
- [x] Enforce spaced structural assignment delimiter ` = ` for `share`, slugged `arrange`, and named ranges.
- [x] Add and align `src/builtins/rearrange/SPEC.md` as the long-term developer maintenance contract for current DST behaviour.

**Verification**:
- Agent: `cargo test --quiet` passed: 173 tests.
- Agent: `cargo clippy --all-targets --all-features -- -D warnings` passed.
- Agent: `cargo fmt --check` passed.

---

## Progress

**Overall**: 100%

| Phase | Progress | Status |
|-------|----------|--------|
| Phase 1 | 100% | ✅ |
| Phase 2 | 100% | ✅ |
| Phase 3 | 100% | ✅ |
| Phase 4 | 100% | ✅ |
| Phase 5 | 100% | ✅ |

**Recent**:
- 2026-06-29T19:41+08:00: New replacement change created; root `SPEC.md` moved into `reference/DST-SPEC.md`.
- 2026-06-29T20:11+08:00: DST implementation completed; tests, clippy, and fmt passed.
- 2026-06-29T21:54+08:00: External review findings F1-F5 fixed; stale old-v1 model removed; regression tests added.
- 2026-06-29T22:26+08:00: Follow-up ambiguity fixed: structural `=` now requires ` = `, and unslugged arrange paths containing `=` fail instead of retargeting.
- 2026-06-29T22:42+08:00: Added/rewrote `src/builtins/rearrange/SPEC.md` as a context-portable maintenance contract aligned with current parser/planner/apply behaviour.
