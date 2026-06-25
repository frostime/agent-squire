---
change: "data-toc"
updated: "2026-06-26T00:19+08:00"
---

# Tasks

## Legend
`[ ]` Todo | `[x]` Done

## Tasks

### Phase 1: JSON/JSONL MVP ✅
- [x] Add `DataToc` CLI command and `datatoc` alias in `src/cli.rs` and `src/builtins/mod.rs` for `feat(cli): Add data-toc command surface`.
- [x] Create `src/builtins/data_toc/mod.rs` with args, `--prompt`, format detection, budget profiles, and top-level `run` flow per `design.md`.
- [x] Implement JSON analysis in `src/builtins/data_toc/mod.rs` for `feat(data-toc): Implement Phase 1 JSON TOC`.
- [x] Implement JSONL analysis in `src/builtins/data_toc/mod.rs` for `feat(data-toc): Implement Phase 1 JSONL TOC`.
- [x] Implement compact and JSON envelope rendering in `src/builtins/data_toc/mod.rs` for `BC-4` and `BC-5`.
- [x] Add `tests/data_toc.rs` covering prompt, alias, JSON output, JSONL groups, JSON envelope, and invalid JSONL line errors.
- [x] Update `README.md` and `CHANGELOG.md` for the new user-visible command.
**Verification**:
- Agent: `cargo fmt` succeeds.
- Agent: `cargo test data_toc` succeeds.
- Agent: `cargo test` succeeds or unrelated failures are documented.
- Agent: `cargo clippy --all-targets --all-features -- -D warnings` succeeds or unrelated failures are documented.
- Agent: sample `cargo s -- data-toc <json>` output contains `format=json`, normalized `[]`, and uncertainty notes.
- Agent: sample `cargo s -- data-toc <jsonl> --format jsonl` output contains `format=jsonl`, `Record groups`, and `first_line`.
- Agent: sample `cargo s -- data-toc --prompt` prints the guide and exits 0.
**User Check**:
1. BC-1: Run `asq data-toc --prompt` → guide explains when to use the command and how to interpret output.
2. BC-1: Run `asq datatoc sample.json` → alias behaves like `asq data-toc sample.json`.
3. BC-2: Run `asq data-toc sample.json` → output summarizes JSON structure without raw values and collapses array indexes to `[]`.
4. BC-3: Run `asq data-toc sample.jsonl --format jsonl` → output shows record groups with representative `first_line` values.
5. BC-5: Run `asq --print json data-toc sample.json` → JSON envelope includes `ok`, `command`, `data`, `warnings`, and `meta`.

### Phase 2: YAML via yq ⏳
- [ ] Extend `DataFormat` and CLI help in `src/builtins/data_toc/mod.rs` to support `yaml`.
- [ ] Add external `yq` detection and YAML-to-JSON conversion path in `src/builtins/data_toc/mod.rs`.
- [ ] Add YAML compact/JSON metadata noting `format=yaml`, `parsed_as=json`, and approximation warnings.
- [ ] Add tests in `tests/data_toc.rs` for missing-`yq` behavior and gated YAML conversion behavior.
**Verification**:
- Agent: missing `yq` path returns a direct non-zero error for YAML input.
- Agent: when `yq` is available, YAML input uses the JSON analysis path and marks approximation.
- Agent: `cargo test data_toc` succeeds.
**User Check**:
1. BC-6: Run `asq data-toc compose.yaml` without `yq` → error states YAML support requires `yq`.
2. BC-6: Run `asq data-toc compose.yaml` with `yq` → output says `format=yaml parsed_as=json`.

### Phase 3: Heuristics and examples ⏳
- [ ] Add dynamic key compression in `src/builtins/data_toc/mod.rs` while preserving existing JSON/JSONL output contracts.
- [ ] Improve JSONL discriminator labeling in `src/builtins/data_toc/mod.rs` using the design candidate fields after shape grouping.
- [ ] Improve `suggested_reads` generation in `src/builtins/data_toc/mod.rs` for JSON and JSONL paths.
- [ ] Add `--examples` with truncation/redaction in `src/builtins/data_toc/mod.rs`.
- [ ] Extend `tests/data_toc.rs` for dynamic keys, discriminator labels, suggested reads, and redacted examples.
**Verification**:
- Agent: dynamic-key fixture renders `{dynamic_key}` instead of many sibling keys.
- Agent: JSONL fixture with stable `type` values renders `type=<value>` group labels.
- Agent: `--examples` fixture truncates/redacts values and remains off by default.
- Agent: `cargo test data_toc` succeeds.
**User Check**:
1. BC-6: Run `asq data-toc dynamic-keys.json` → repeated sibling keys are compressed as `{dynamic_key}`.
2. BC-6: Run `asq data-toc sample.json --examples` → output contains limited redacted examples; default output still hides values.

### Feedback Tasks

- (none yet)

---

## Progress

**Overall**: 33%

| Phase | Progress | Status |
|-------|----------|--------|
| Phase 1 | 7/7 | ✅ |
| Phase 2 | 0/4 | ⏳ |
| Phase 3 | 0/5 | ⏳ |

**Recent**:
- 2026-06-26T00:19+08:00 Planned phased execution tasks after design confirmation.
- 2026-06-26T00:20+08:00 Registered `data-toc` CLI command and `datatoc` alias; `cargo check` passed.
- 2026-06-26T00:38+08:00 Implemented args, prompt, format detection, budget profiles, and run flow; `data-toc --prompt` and alias prompt verified.
- 2026-06-26T00:39+08:00 Implemented JSON TOC; sample output showed object tree, array `[]`, presence counts, and suggested read.
- 2026-06-26T00:40+08:00 Implemented JSONL TOC; sample output showed record groups, discriminator labels, and `first_line`; invalid JSONL reports line number.
- 2026-06-26T00:41+08:00 Verified compact rendering and JSON envelope shape for `data-toc`.
- 2026-06-26T00:42+08:00 Added `tests/data_toc.rs`; focused `cargo test --test data_toc` passed.
- 2026-06-26T00:43+08:00 Updated README and changelog for Phase 1 `data-toc`.
