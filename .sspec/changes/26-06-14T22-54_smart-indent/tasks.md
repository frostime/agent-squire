---
change: "smart-indent"
updated: "2026-06-15T01:05+08:00"
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

### Phase 0: Clarify And Redesign ✅
- [x] Re-review current implementation against original spec/design.
- [x] Clarify smart-indent semantics with user: block base-indent migration, strict default, blank-line handling, idempotency, options API, multi-candidate ambiguity.
- [x] Record correction revision `revisions/001-redefine-smart-indent.md`.
- [x] Rewrite `spec.md` and `design.md` as the new prediction contract.
- [x] Align rewritten design with user before implementation planning.
**Verification**: User confirmed rewritten spec/design and allowed direct implementation after planning.

### Phase 1: API And Data Model ✅
- [x] Update `src/builtins/patch_edit/model.rs` with options and `indent_from`/`indent_to` result metadata.
- [x] Update `src/builtins/patch_edit/match_apply.rs` public entrypoints to preserve old API and add options API.
- [x] Update `src/builtins/patch_edit/mod.rs` CLI calls to use options API.
**Verification**: Existing non-smart-indent tests compile through the old 3-arg API.

### Phase 2: Smart-Indent Core ✅
- [x] Update `src/builtins/patch_edit/text.rs` with base-indent and migration helpers per design.
- [x] Replace smart-indent matching in `src/builtins/patch_edit/match_apply.rs` with base-indent candidate detection.
- [x] Add adjusted REPLACE application and already-applied checks for `--smart-indent`.
**Verification**: Targeted smart-indent integration tests pass.

### Phase 3: Tests And Output ✅
- [x] Replace stale smart-indent tests in `tests/patch_edit_compat.rs` with behavior tests from design matrix.
- [x] Ensure `src/builtins/patch_edit/output.rs` compact output includes useful smart-indent metadata.
**Verification**: `cargo test --test patch_edit_compat` passes.

### Phase 4: Final Validation ✅
- [x] Run `cargo test --test patch_edit_compat`.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] Run broader validation and record unrelated baseline failures.
**Verification**: Targeted tests and clippy pass; full `cargo test` and `cargo fmt --check` have unrelated `read_range` baseline failures.

---

## Progress

**Overall**: 100%

| Phase | Progress | Status |
|-------|----------|--------|
| Phase 0 | 5/5 | ✅ |
| Phase 1 | 3/3 | ✅ |
| Phase 2 | 3/3 | ✅ |
| Phase 3 | 2/2 | ✅ |
| Phase 4 | 3/3 | ✅ |

**Recent**:
- [2026-06-15T00:37+08:00] Re-entered Clarify after review found design/implementation mismatch; rewrote smart-indent spec/design around block base-indent migration.
- [2026-06-15T00:45+08:00] Planned implementation phases and began Phase 1.
- [2026-06-15T01:05+08:00] Completed smart-indent rewrite, targeted tests, and clippy; full validation has unrelated `read_range` baseline failures.
