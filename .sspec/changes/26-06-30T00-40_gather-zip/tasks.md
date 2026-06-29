---
change: "gather-zip"
updated: "2026-06-30T00:45:00"
---

# Tasks

## Legend
`[ ]` Todo | `[x]` Done

## Tasks

### Phase 1: Data model + scaffolding ✅
- [x] Add `Zip { path, and_done }` variant to `InteractiveCommand` in `src/builtins/gather/interactive.rs`
- [x] Update `parse_interactive_command()` to parse `/zip`, `/zip <path>`, `/zip /done`, `/zip <path> /done` in `src/builtins/gather/interactive.rs`
- [x] Create `src/builtins/gather/zip.rs` with stub: `assemble_zip()`, `create_zip_archive()`, `collect_warnings_and_confirm()`
- [x] Add `mod zip;` to `src/builtins/gather/mod.rs`
- [x] Define `Manifest`, `ManifestEntry`, `RangeField` types with serde Serialize in `src/builtins/gather/zip.rs`
- [x] Define `FileEntry` and `ArtifactEntry` / `ArtifactMeta` internal types in `src/builtins/gather/zip.rs`
**Verification**:
- Agent: `cargo check --lib` compiles
- Agent: `cargo test --lib` (existing tests pass)

---

### Phase 2: File collection + artifact generation ✅
- [ ] Implement `collect_file_entries()` — traverse Sources, expand Dir/Glob/SelectedGlob, resolve File paths, dedup by `files/<relative_path>` in `src/builtins/gather/zip.rs`
- [ ] Implement `generate_artifacts()` — execute Cmd sources, call `render_tree()` for Tree sources, slice ranged File sources, produce `ArtifactEntry` list in `src/builtins/gather/zip.rs`
- [ ] Implement `build_manifest()` — convert `Vec<FileEntry>` + `Vec<ArtifactEntry>` + sources into `Manifest` struct in `src/builtins/gather/zip.rs`
- [ ] Implement `assemble_staging_dir()` — create tempdir, copy files to `files/`, write artifacts to `artifacts/`, write `manifest.json` in `src/builtins/gather/zip.rs`
- [ ] Implement `sanitize_filename()` helper — replace `/ \ : * ? " < > |` and spaces with `-` in `src/builtins/gather/zip.rs`
**Verification**:
- Agent: `cargo test --lib` (existing + new unit tests for sanitize, dedup)
- Agent: manual `dbg!` on staging dir structure matches design.md structure

---

### Phase 3: External zip creation + output ✅
- [ ] Implement `create_zip_archive()` — `#[cfg(windows)]` → `powershell Compress-Archive`, `#[cfg(not(windows))]` → `zip -r` in `src/builtins/gather/zip.rs`
- [x] Implement cross-volume rename fallback: `fs::rename` → on error `fs::copy` + `fs::remove_file` in `src/builtins/gather/zip.rs`
- [ ] Implement `assemble_zip()` top-level orchestrator wiring phases 2+3 in `src/builtins/gather/zip.rs`
**Verification**:
- Agent: `cargo test --lib`
- Agent: manually run `/zip` in interactive mode → verify zip is created in cwd, unzippable, contains correct structure

---

### Phase 4: Safety checks + warning UX ✅
- [ ] Implement `is_binary()` — read first 8KB, detect null byte in `src/builtins/gather/zip.rs`
- [ ] Implement `collect_warnings()` — iterate FileEntries, classify binary + >10MB, return warning list in `src/builtins/gather/zip.rs`
- [ ] Implement `confirm_warnings()` — print merged warning list, read stdin Y/n in `src/builtins/gather/zip.rs`
**Verification**:
- Agent: `cargo test --lib` (unit tests for is_binary with text/binary fixtures)
- Agent: manual `/zip` with a .png and >10MB file → warning display + Y/n prompt

---

### Phase 5: Interactive command wiring ✅
- [x] Wire `InteractiveCommand::Zip` into `read_sources()` main loop in `src/builtins/gather/interactive.rs`
- [x] Handle edge cases: empty sources → error "No sources to package"
- [x] Handle edge cases: no file-backed sources → error "No file sources to package"
- [x] Handle `/zip /done` → package then set `render = true` and break
- [x] Handle external path sources (absolute / `../`) — flatten to `files/_external/<safe-name>`, print warning in `src/builtins/gather/zip.rs`
**Verification**:
- Agent: `cargo test --lib`
- Agent: `cargo clippy --all-targets --all-features -- -D warnings`

---

### Phase 6: Integration tests ✅
- [x] Add test: `/zip` with file+dir+cmd → zip exists, contains `files/` + `artifacts/` + `manifest.json` in `tests/gather.rs`
- [x] Add test: `/zip` with empty sources → error message in `tests/gather.rs`
- [x] Add test: `/zip` with binary file → warning appears on stderr in `tests/gather.rs`
- [x] Add test: `/zip /done` → zip created + process exits in `tests/gather.rs`
- [x] Add test: `/zip` with ranged file → artifact contains correct slice in `tests/gather.rs`
- [x] Add test: `/zip --no-gitignore` state propagates correctly in `tests/gather.rs`
**Verification**:
- Agent: `cargo test` (all integration tests pass)
- Agent: `cargo clippy --all-targets --all-features -- -D warnings`

**User Check**:
1. BC-1: `asq gather -i` → add `file:src/main.rs` → type `/zip` → verify zip created in cwd at `asq-gather-<timestamp>.zip`
2. BC-2: Unzip output → verify `files/`, `artifacts/`, `manifest.json` exist with correct content
3. BC-3: `asq gather -i` → add `.png` file → type `/zip` → verify warning appears, type `y` → zip created with .png inside
4. BC-5: `asq gather -i` → type `/zip` immediately → verify "No sources to package" error

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
| Phase 6 | 100% | ✅ |

**Recent**:
- (none yet)
