---
change: "compose"
updated: "2026-06-10T15:12+08:00"
---

# Tasks

## Legend
`[ ]` Todo | `[x]` Done

## Tasks

<!-- MUST organize by phases. Each task <2h, independently testable.
Phase emoji: ⏳ pending | 🚧 in progress | ✅ done

### Phase 1: <name> ⏳
- [ ] Task description `path/file.py`
- [ ] Task description `path/file.py`
**Verification**: <how to verify this phase>

### Feedback Tasks (→ [001-separate-compile-render-phases](./revisions/001-separate-compile-render-phases.md)) ✅
- [x] Add `src/builtins/compose/compile.rs` for static semantic analysis and compiled source listing.
- [x] Add `src/builtins/compose/render.rs` and move render/eval flow out of `src/builtins/compose/mod.rs`.
- [x] Move normalization types/functions out of `src/builtins/compose/modifiers.rs` where they are compile-stage concerns.
- [x] Update compose tests so `--check` catches compile-time conflicts without source evaluation.
- [x] Re-run targeted validation and update REVIEW state.

Use this section for review/feedback tasks that still belong to the current change.

If accepted feedback changes scope/design:
- **Design phase**: update `spec.md` / `design.md` directly, then add tasks here.
- **Plan/Implement/Review** (spec locked): create `revisions/NNN-*.md` FIRST, then update this section. Do NOT edit `spec.md` / `design.md`.

The section header MUST link the corresponding revision file (relative path).
If the work belongs in a new follow-up or replacement change, the agent MUST NOT put it here unless the user has first approved that direction via `@align`.
-->

### Phase 1: CLI Scaffold And Output Shell ✅
- [x] Register `compose` in `src/cli.rs` and `src/builtins/mod.rs` per **Feat A**.
- [x] Create `src/builtins/compose/mod.rs`, `model.rs`, and `output.rs` with args, status structs, `--prompt`, and output target selection per **Feat E/F**.
- [x] Add initial CLI smoke tests in `tests/compose.rs` for help, `--prompt`, mutual exclusion, and default temp path reporting.
**Verification**: `cargo test --test compose compose_cli` passes; `cargo test --test clap` passes.

### Phase 2: Template Parser ✅
- [x] Implement `src/builtins/compose/parser.rs` for segments, multiline interpolation, escaping, JSON body parsing, and source locations per **Feat B**.
- [x] Implement command role classification and validation in `src/builtins/compose/model.rs` per **Feat D**.
- [x] Add parser tests in `src/builtins/compose/parser.rs` for source-first, no-arg colon omission, quoted `|>`/`}}`, multiline rules, and validation errors.
**Verification**: `cargo test compose::parser` passes.

### Phase 3: Source Resolution ✅
- [x] Implement `src/builtins/compose/text.rs` for宽松 decoding, binary detection, UTF-8-without-BOM output encoding, and truncation helpers.
- [x] Implement `src/builtins/compose/sources.rs` for `stdin`, `file`, `env`, and guarded `exec` per **Feat C**.
- [x] Add source tests for cached stdin, terminal stdin failure, file/env 404, encoding fallback, exec disabled, exec success, exec non-zero, and timeout.
**Verification**: `cargo test compose::sources` passes on Windows/MSYS and does not hang on terminal stdin cases.

### Phase 4: Stage Command Engine ✅
- [x] Implement `src/builtins/compose/modifiers.rs` for runtime/stream/policy normalization and text transforms per **Feat D**.
- [x] Add modifier tests for line/char ranges, head/tail, trim/oneline/indent, max limits, stream selector normalization, duplicate controls, and fallback precedence.
- [x] Add failure-case tests for range, binary, encoding, limit, modifier, timeout, error, and 404 recovery.
**Verification**: `cargo test compose::modifiers` passes.

### Phase 5: Render Orchestration And Integration ✅
- [x] Wire render flow in `src/builtins/compose/mod.rs` for `--check`, `--list-sources`, temp output, `-o/--output`, `--stdout`, and structured compact/json status.
- [x] Add integration tests in `tests/compose.rs` for default temp output, explicit output overwrite behavior, stdout body mode, no rendered body in JSON, check/list-sources no side effects, and multiline templates.
- [x] Add integration tests for end-to-end templates using stdin/file/env/exec with modifiers and fallbacks.
**Verification**: `cargo test --test compose` passes.

### Phase 6: Docs And Full Validation ✅
- [x] Update `README.md` with `compose` command, output contract, template syntax, and examples.
- [x] Update `CHANGELOG.md` with the new `compose` command.
- [x] Run final validation and record unrelated baseline failures.
**Verification**: `cargo test --test compose`, `cargo test --test clap`, and `cargo clippy --all-targets --all-features -- -D warnings` pass. Full `cargo test` is blocked by existing `tests/md_backlinks.rs` failure; `cargo fmt --check` is blocked by existing `src/builtins/imgweb/mod.rs` formatting diff.

### Feedback Tasks (→ [002-exec-spill-artifacts-and-timeout-schema](./revisions/002-exec-spill-artifacts-and-timeout-schema.md)) ✅
- [x] Add artifact/error/status data model and compose JSON metadata schema in `src/builtins/compose/model.rs` and `src/builtins/compose/output.rs`.
- [x] Add `--max-spill-bytes` with a 128MiB per-render budget in `src/builtins/compose/mod.rs`.
- [x] Refactor `exec:` capture in `src/builtins/compose/sources.rs` to concurrently drain stdout/stderr, spill excess output, and avoid pipe deadlock.
- [x] Enforce `--total-timeout` as a render-wide deadline in `src/builtins/compose/render.rs` and pass remaining budget to exec resolution.
- [x] Route template-load failures through compose structured errors and preserve artifacts on truncation errors.
- [x] Update `README.md`, `CHANGELOG.md`, and targeted compose tests.
**Verification**: `cargo test --test compose`, targeted compose unit tests, and `cargo clippy --all-targets --all-features -- -D warnings` pass; `cargo fmt` is run or baseline blocker is recorded.

---

## Progress

**Overall**: 100%

| Phase | Progress | Status |
|-------|----------|--------|
| Phase 1 | 3/3 | ✅ |
| Phase 2 | 3/3 | ✅ |
| Phase 3 | 3/3 | ✅ |
| Phase 4 | 3/3 | ✅ |
| Phase 5 | 3/3 | ✅ |
| Phase 6 | 3/3 | ✅ |
| Feedback 001 | 5/5 | ✅ |
| Feedback 002 | 6/6 | ✅ |

**Recent**:
- [2026-06-10T01:46+08:00] Planned implementation phases from confirmed design.
- [2026-06-10T01:48+08:00] Started Phase 1 implementation.
- [2026-06-10T02:05+08:00] Completed compose implementation and targeted validation; recorded unrelated baseline failures.
- [2026-06-10T02:36+08:00] Started revision 001 to separate parse/compile/render phases.
- [2026-06-10T02:52+08:00] Completed revision 001 refactor and targeted validation.
- [2026-06-10T15:12+08:00] Started revision 002 for exec spill artifacts, render-wide timeout semantics, and compose JSON schema metadata.
- [2026-06-10T15:31+08:00] Completed revision 002 implementation and validation; returned change to REVIEW.
