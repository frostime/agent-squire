---
change: "compose"
updated: "2026-06-10T01:46+08:00"
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

### Feedback Tasks (→ [NNN-description](./revisions/NNN-description.md))
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

**Recent**:
- [2026-06-10T01:46+08:00] Planned implementation phases from confirmed design.
- [2026-06-10T01:48+08:00] Started Phase 1 implementation.
- [2026-06-10T02:05+08:00] Completed compose implementation and targeted validation; recorded unrelated baseline failures.
