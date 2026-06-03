---
change: "md-backlinks"
updated: "2026-06-03T20:58+08:00"
---

# Tasks

## Legend
`[ ]` Todo | `[x]` Done

## Tasks

### Phase 1: TDD fixtures and expected behavior ✅
- [x] Add `tests/md_backlinks.rs` with JSON behavior tests from `design.md`.
- [x] Add compact output assertions in `tests/md_backlinks.rs`.
- [x] Add corpus filtering assertions in `tests/md_backlinks.rs` for default gitignore, `--no-gitignore`, and explicit ignored file inclusion.
**Verification**: `cargo test --test md_backlinks` fails because `md-backlinks` command is not implemented yet, and failures correspond to missing command/behavior rather than malformed test setup.

### Phase 2: CLI entry and shared link graph core ✅
- [x] Update `src/cli.rs` to register and dispatch `md-backlinks` per spec.
- [x] Update `src/builtins/mod.rs` to export `md_backlinks`.
- [x] Add `src/builtins/md_links/graph.rs` or equivalent shared helper for resolved link edges.
- [x] Adjust `src/builtins/md_links/{model,parse,resolve,sources}.rs` visibility only as needed for shared use.
**Verification**: `cargo test --test md_links` passes unchanged; `cargo test --test md_backlinks` reaches backlink behavior assertions instead of clap “unrecognized subcommand”.

### Phase 3: Backlink command implementation ✅
- [x] Create `src/builtins/md_backlinks/mod.rs` with args, focus normalization, corpus discovery, backlink grouping, and output per `design.md`.
- [x] Reuse `md_links` parser/resolver through shared core; do not add a second Markdown link parser.
- [x] Implement JSON envelope metadata for workspace, `from`, ignore policy, and extensions.
- [x] Implement compact output grouped by focus page.
**Verification**: `cargo test --test md_backlinks` passes.

### Phase 4: Quality gates and checkpoint ✅
- [x] Run `cargo test`.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] Run `cargo fmt`.
- [x] Update `memory.md` state/milestones and task progress.
- [x] Commit implementation checkpoint on `wip/md-backlinks`.
**Verification**: `cargo test` passes. `cargo clippy --all-targets --all-features -- -D warnings` was run and fails on pre-existing unrelated lint errors in `imgweb`, `info`, `patch_edit`, `tree`, and `external`; the user requested handling those in a separate branch instead of fixing them in this change.

### Feedback Tasks (→ [001-use-cwd-as-backlinks-path-root](./revisions/001-use-cwd-as-backlinks-path-root.md)) ✅
- [x] Remove `--workspace` from `src/builtins/md_backlinks/mod.rs` and use `ctx.cwd` as the root.
- [x] Update focus normalization, corpus discovery, and JSON metadata to use the effective CWD/root semantics.
- [x] Update `tests/md_backlinks.rs` for global `--cwd` behavior and absence of `md-backlinks --workspace`.
- [x] Re-run focused tests and `cargo test`; keep unrelated clippy baseline out of scope.
**Verification**: `cargo test --test md_backlinks`, `cargo test --test md_links`, and `cargo test` pass. `md-backlinks --workspace` is rejected by clap. `cargo clippy --all-targets --all-features -- -D warnings` still fails only on unrelated baseline files recorded earlier.

---

## Progress

**Overall**: 100%

| Phase | Progress | Status |
|-------|----------|--------|
| Phase 1 | 3/3 | ✅ |
| Phase 2 | 4/4 | ✅ |
| Phase 3 | 4/4 | ✅ |
| Phase 4 | 5/5 | ✅ |

**Recent**:
- 2026-06-03T20:58+08:00 Planned TDD implementation phases from approved design.
- 2026-06-03T21:00+08:00 Added failing `tests/md_backlinks.rs`; `cargo test --test md_backlinks` fails only because `md-backlinks` is unimplemented.
- 2026-06-03T21:03+08:00 Implemented CLI, shared graph helper, backlink command, JSON/compact output; `cargo test --test md_links` and `cargo test --test md_backlinks` pass.
- 2026-06-03T21:05+08:00 `cargo test` passes; clippy gate is blocked by unrelated pre-existing lint errors outside the md-backlinks change.
- 2026-06-03T21:06+08:00 Committed implementation checkpoint on `wip/md-backlinks`.
- 2026-06-03T21:09+08:00 Recorded unrelated clippy baseline issue as a local ignored `.sspec/tmp` backlog file; current change moved to REVIEW.
- 2026-06-03T21:24+08:00 Added revision 001 to remove `md-backlinks --workspace` and use effective CWD as the single backlinks path root.
- 2026-06-03T21:29+08:00 Implemented revision 001; focused tests and `cargo test` pass, clippy remains blocked by unrelated baseline lints.
- 2026-06-03T21:53+08:00 Deduplicated repeated focus pages, added regression test, cleaned spec trailing whitespace, and accepted change as DONE for merge.
