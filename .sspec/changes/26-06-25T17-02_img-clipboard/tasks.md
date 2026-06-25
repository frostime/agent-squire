---
change: "img-clipboard"
updated: "2026-06-25T18:06+08:00"
---

# Tasks

## Legend
`[ ]` Todo | `[x]` Done

## Tasks

### Phase 1: CLI surface and dependencies ✅
- [x] Update `Cargo.toml` / `Cargo.lock` — add dependencies from `design.md`.
- [x] Update `src/builtins/mod.rs` — register new `img` builtin module.
- [x] Update `src/cli.rs` — add public `img` subcommand, keep `imgweb` executable as hidden legacy, dispatch both paths.
- [x] Update `src/cli.rs` list output — exclude hidden legacy `imgweb` from primary discovery if clap metadata still returns it.
**Verification**:
- Agent: `cargo check` succeeds.
- Agent: `cargo run --quiet --bin asq -- --help` shows `img` and does not show `imgweb`.
- Agent: `cargo run --quiet --bin asq -- list` shows `img` and does not show `imgweb`.
- Agent: `cargo run --quiet --bin asq -- imgweb --help` still exits 0.
**User Check**:
1. BC-1: `asq --help` → `img` is the public image command; `imgweb` is not advertised.

### Phase 2: Clipboard image save implementation ✅
- [x] Create `src/builtins/img/mod.rs` — implement `ImgArgs`, mode selection, and `--web` delegation per `design.md`.
- [x] Implement clipboard read path in `src/builtins/img/mod.rs` — use `arboard` to read an image and convert failure cases into concise anyhow errors.
- [x] Implement PNG write helpers in `src/builtins/img/mod.rs` — save RGBA clipboard image under `agent-temp/images/clip-...`.
- [x] Implement output formatting in `src/builtins/img/mod.rs` — path for non-JSON modes, `Envelope` for JSON.
- [x] Add focused unit tests in `src/builtins/img/mod.rs` for pure helpers: PNG writing, URI formatting, and invalid buffer handling.
**Verification**:
- Agent: `cargo test img` succeeds.
- Agent: `cargo run --quiet --bin asq -- img --help` shows clipboard default and `--web` mode.
- Agent: with an image in clipboard, `cargo run --quiet --bin asq -- img` prints an existing `.png` path.
- Agent: with an image in clipboard, `cargo run --quiet --bin asq -- --json img` prints JSON with `data.path`, `data.uri`, `data.mime == "image/png"`, and `data.size_bytes > 0`.
**User Check**:
1. BC-2: copy/screenshot an image, run `asq img` → stdout is a local PNG path that exists.
2. BC-3: run `asq img --web --no-open` → starts the existing local web UI server.

### Phase 3: Docs and quality gates ✅
- [x] Update `README.md` — document `img` as the public image workflow and remove `imgweb` as recommended entrypoint.
- [x] Update `CHANGELOG.md` if an Unreleased/current-version section exists for user-visible changes.
- [x] Run repository formatting, tests, and lints.
**Verification**:
- Agent: `cargo fmt --check` succeeds.
- Agent: `cargo test` succeeds.
- Agent: `cargo clippy --all-targets --all-features -- -D warnings` succeeds.
- Agent: `README.md` command table/docs advertise `img`, not `imgweb`.
**User Check**:
1. BC-1/BC-3: docs point users to `asq img` and `asq img --web`.

### Feedback Tasks (→ [001-windows-dib-fallback](./revisions/001-windows-dib-fallback.md))

- [x] Diagnose Windows clipboard formats for user-reported pasteable image that `arboard` could not convert.
- [x] Update `src/builtins/img/mod.rs` — add Windows `CF_DIB` / `CF_DIBV5` fallback decode path.
- [x] Update `Cargo.toml` / `Cargo.lock` — add explicit Windows clipboard and BMP decode support.
- [x] Re-run targeted runtime check and quality gates.

---

## Progress

**Overall**: 100%

| Phase | Progress | Status |
|-------|----------|--------|
| Phase 1 | 4/4 | ✅ |
| Phase 2 | 5/5 | ✅ |
| Phase 3 | 3/3 | ✅ |

**Recent**:
- [2026-06-25T17:09+08:00] Plan created after design confirmation.
- [2026-06-25T17:20+08:00] Implemented CLI/deps and clipboard PNG save module; verified `cargo check`, help/list visibility, `imgweb --help`, and `cargo test img`.
- [2026-06-25T17:51+08:00] Docs updated and quality gates passed; direct clipboard runtime check left to user review to avoid reading/saving current clipboard contents.
- [2026-06-25T18:06+08:00] Review feedback fixed: added Windows DIB fallback; generated test bitmap saved successfully via `asq img`; full quality gates passed again.
