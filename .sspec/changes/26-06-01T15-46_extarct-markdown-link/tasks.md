---
change: "extarct-markdown-link"
updated: "2026-06-01T16:07+08:00"
---

# Tasks

## Legend
`[ ]` Todo | `[x]` Done

## Tasks

### Phase 1: CLI skeleton + module structure ✅
- [x] Add `src/builtins/md_links/` module files per `design.md`: `mod.rs`, `model.rs`, `sources.rs`, `parse.rs`, `resolve.rs`, `output.rs`.
- [x] Register `md-links` / `mdlinks` in `src/cli.rs` and export module in `src/builtins/mod.rs`.
- [x] Implement argument parsing and command orchestration in `src/builtins/md_links/mod.rs` per spec Feat A.
**Verification**: `cargo test clap_definition_is_valid`; `cargo run --bin squire -- md-links --help` shows sources and `--workspace`. Passed 2026-06-01T16:16+08:00.

### Phase 2: Source expansion + data model ✅
- [x] Define serializable output structs/enums in `src/builtins/md_links/model.rs` per `design.md` Output Contract.
- [x] Implement file/directory/glob source expansion in `src/builtins/md_links/sources.rs`, mirroring `md-toc` behavior where compatible.
- [x] Add behavior tests in `tests/md_links.rs` for file, directory, glob, missing source warnings/errors.
**Verification**: `cargo test --test md_links` passed 2026-06-01T16:21+08:00; source discovery and JSON envelope counts covered.

### Phase 3: Reference parsing ✅
- [x] Implement fenced-code-aware scanners in `src/builtins/md_links/parse.rs` for markdown links/images, wiki links, inline code paths, angle refs, and SiYuan block refs per `design.md` Syntax Coverage.
- [x] Add behavior tests verifying representative syntaxes through `asq --print json md-links`, including fenced-code skipping.
- [x] Add focused pure parser tests only for edge cases that are hard to express through CLI fixtures.
**Verification**: `cargo test --test md_links` passed 2026-06-01T16:21+08:00; parser tests assert occurrence fields without full-output snapshots.

### Phase 4: Classification + path resolution ✅
- [x] Implement target classification and URL scheme handling in `src/builtins/md_links/resolve.rs` per `design.md` Path Classification.
- [x] Implement file resolution: source-relative, workspace-relative, `/src` workspace-first fallback, OS absolute fallback, slash normalization, fragment/query stripping for existence checks, wiki `.md` fallback.
- [x] Add behavior tests for workspace refs across `[]()`, `[[]]`, inline code, backslash paths, missing files, and SiYuan URL/block refs.
**Verification**: `cargo test --test md_links` passed 2026-06-01T16:21+08:00; tests assert `target_type`, `resolved`, and `exists` for resolution rules.

### Phase 5: Output + integration polish ✅
- [x] Implement JSON envelope and compact grouped line protocol in `src/builtins/md_links/output.rs` per `design.md` Output Contract.
- [x] Wire warnings/meta/summary totals in `src/builtins/md_links/mod.rs`.
- [x] Update `CHANGELOG.md` with the new `md-links` command.
- [x] Run formatting and full quality gates.
**Verification**: `cargo fmt` and `cargo test` passed 2026-06-01T16:24+08:00. `cargo clippy --all-targets --all-features -- -D warnings` failed only on pre-existing unrelated lints outside `src/builtins/md_links/`; see Recent.

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
- 2026-06-01T16:07+08:00 — Plan created after design confirmation and WIP branch checkpoint.
- 2026-06-01T16:16+08:00 — Phase 1 complete; clap definition and help output verified.
- 2026-06-01T16:21+08:00 — Core `md-links` behavior implemented with 6 behavior-oriented integration tests passing.
- 2026-06-01T16:24+08:00 — `cargo fmt` and `cargo test` passed; clippy gate blocked by unrelated pre-existing lints in `imgweb`, `info`, `patch_edit`, `tree`, and `external`.
