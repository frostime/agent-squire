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

### Phase 1: CLI Scaffold And Output Shell ⏳
- [ ] Register `compose` in `src/cli.rs` and `src/builtins/mod.rs` per **Feat A**.
- [ ] Create `src/builtins/compose/mod.rs`, `model.rs`, and `output.rs` with args, status structs, `--prompt`, and output target selection per **Feat E/F**.
- [ ] Add initial CLI smoke tests in `tests/compose.rs` for help, `--prompt`, mutual exclusion, and default temp path reporting.
**Verification**: `cargo test --test compose compose_cli` passes; `cargo test --test clap` passes.

### Phase 2: Template Parser ⏳
- [ ] Implement `src/builtins/compose/parser.rs` for segments, multiline interpolation, escaping, JSON body parsing, and source locations per **Feat B**.
- [ ] Implement command role classification and validation in `src/builtins/compose/model.rs` per **Feat D**.
- [ ] Add parser tests in `src/builtins/compose/parser.rs` for source-first, no-arg colon omission, quoted `|>`/`}}`, multiline rules, and validation errors.
**Verification**: `cargo test compose::parser` passes.

### Phase 3: Source Resolution ⏳
- [ ] Implement `src/builtins/compose/text.rs` for宽松 decoding, binary detection, UTF-8-without-BOM output encoding, and truncation helpers.
- [ ] Implement `src/builtins/compose/sources.rs` for `stdin`, `file`, `env`, and guarded `exec` per **Feat C**.
- [ ] Add source tests for cached stdin, terminal stdin failure, file/env 404, encoding fallback, exec disabled, exec success, exec non-zero, and timeout.
**Verification**: `cargo test compose::sources` passes on Windows/MSYS and does not hang on terminal stdin cases.

### Phase 4: Stage Command Engine ⏳
- [ ] Implement `src/builtins/compose/modifiers.rs` for runtime/stream/policy normalization and text transforms per **Feat D**.
- [ ] Add modifier tests for line/char ranges, head/tail, trim/oneline/indent, max limits, stream selector normalization, duplicate controls, and fallback precedence.
- [ ] Add failure-case tests for range, binary, encoding, limit, modifier, timeout, error, and 404 recovery.
**Verification**: `cargo test compose::modifiers` passes.

### Phase 5: Render Orchestration And Integration ⏳
- [ ] Wire render flow in `src/builtins/compose/mod.rs` for `--check`, `--list-sources`, temp output, `-o/--output`, `--stdout`, and structured compact/json status.
- [ ] Add integration tests in `tests/compose.rs` for default temp output, explicit output overwrite behavior, stdout body mode, no rendered body in JSON, check/list-sources no side effects, and multiline templates.
- [ ] Add integration tests for end-to-end templates using stdin/file/env/exec with modifiers and fallbacks.
**Verification**: `cargo test --test compose` passes.

### Phase 6: Docs And Full Validation ⏳
- [ ] Update `README.md` with `compose` command, output contract, template syntax, and examples.
- [ ] Update `CHANGELOG.md` with the new `compose` command.
- [ ] Run final validation: `cargo fmt`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`.
**Verification**: All final validation commands pass.

---

## Progress

**Overall**: 0%

| Phase | Progress | Status |
|-------|----------|--------|
| Phase 1 | 0/3 | ⏳ |
| Phase 2 | 0/3 | ⏳ |
| Phase 3 | 0/3 | ⏳ |
| Phase 4 | 0/3 | ⏳ |
| Phase 5 | 0/3 | ⏳ |
| Phase 6 | 0/3 | ⏳ |

**Recent**:
- [2026-06-10T01:46+08:00] Planned implementation phases from confirmed design.
