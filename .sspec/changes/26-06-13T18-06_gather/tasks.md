---
change: "gather"
updated: "2026-06-13T19:00+08:00"
---

# Tasks

## Legend
`[ ]` Todo | `[x]` Done

## Tasks

### Phase 1: CLI skeleton + parser ✅
- [x] Register `gather` in `src/cli.rs` and `src/builtins/mod.rs`.
- [x] Create `src/builtins/gather/mod.rs` with CLI args, prompt text, and orchestration shell.
- [x] Create `src/builtins/gather/model.rs` and `src/builtins/gather/parser.rs` for source types, prefix parsing, auto-detection, and file range parsing.
- [x] Add parser unit tests in `src/builtins/gather/parser.rs`.
**Verification**: `cargo test gather:: --lib` passes parser tests.

### Phase 2: template generation + source expansion ✅
- [x] Create `src/builtins/gather/template.rs` for delimiter rendering and compose JSON body quoting.
- [x] Create `src/builtins/gather/sources.rs` for dir/glob expansion, deterministic sorting, ignore-aware directory walking, and tree rendering.
- [x] Generate compose template for `file`, file ranges, `dir`, `glob`, `tree`, and `cmd` per `design.md`.
- [x] Add unit tests for generated templates and dir/glob grouped output.
**Verification**: `cargo test gather:: --lib` passes template/source tests.

### Phase 3: compose integration + output identity ✅
- [x] Expose minimal compose internals needed by `gather` without changing compose CLI behavior.
- [x] Render generated template with compose options, enabling exec when required.
- [x] Ensure default output path/status uses `asq-gather-*` and JSON command identity uses `gather`.
- [x] Add integration tests in `tests/gather.rs` for stdout, temp output, explicit output, file range, cmd exec, dir/glob grouped output, and ordering contract.
**Verification**: `cargo test --test gather` passes.

### Phase 4: interactive mode ✅
- [x] Create `src/builtins/gather/interactive.rs` for prompt loop, explicit source lines, `cmd:` body line, and selector-only fzf triggers.
- [x] Implement fzf command detection/invocation for file, dir, tree, and glob selectors.
- [x] Add testable seams for fzf selection parsing and source conversion.
**Verification**: `cargo test gather:: --lib` passes interactive seam tests; fzf manual smoke not run in this non-interactive session.

### Phase 5: docs + final verification ✅
- [x] Update `README.md` with `gather` command docs and examples.
- [x] Update `CHANGELOG.md` under Unreleased.
- [x] Run formatter and full project checks.
**Verification**: `cargo fmt`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings` pass.

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
- [2026-06-13T19:00+08:00] Planned implementation phases.
- [2026-06-13T19:10+08:00] Implemented gather CLI skeleton, parser, template generation, source expansion, and unit tests; `cargo test gather:: --lib` passes.
- [2026-06-13T19:25+08:00] Implemented compose integration, interactive fzf selector mode, docs, and integration tests; `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt` pass.
