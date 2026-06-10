# Memory: compose

**Updated**: 2026-06-10T16:58+08:00

## Git Baseline (Immutable)
<!-- Captured during `sspec change new` before any change files are written.
This section records the change starting point in git and MUST NOT be edited or refreshed later. -->

- Captured: before change file creation
- Repository: `D:/Arsenal/PlayCode/agent-squire`
- Branch: `feat/render-compose`
- HEAD: `fbd9d20595bee0aa0d7acf96ff2b0fc81068470d`
- Worktree: `dirty`
- Status Snapshot: raw `git status --short --branch` output

```text
## feat/render-compose
A  .sspec/requests/26-06-10T00-36_composer.md
```

## State
<!-- Where we are and what's next — one to three lines.
This is the resume entry point; the first section an agent reads on cold start. -->

Revision 002 is complete and the change is back in REVIEW. Exec output now drains concurrently with bounded temp spill artifacts; compose JSON includes schema metadata/artifacts; full validation passes.

## Key Files
<!-- Files critical to understanding/continuing this change.
- `path/file` — what it contains, why it matters -->

- `.sspec/changes/26-06-10T01-22_compose/spec.md` — formal change scope and user-visible contract.
- `.sspec/changes/26-06-10T01-22_compose/design.md` — technical design for parser, sources, modifiers, and output behavior.
- `.sspec/changes/26-06-10T01-22_compose/reference/context-composer-prd.md` — original seed PRD retained for traceability.
- `.sspec/spec-docs/compose-template-engine.md` — maintainer contract for compose phase boundaries, source semantics, exec spill artifacts, and output schema.
- `src/cli.rs` — top-level command registration and global `--cwd` / `--print` behavior.
- `src/runtime/input.rs` — existing `@stdin` / `@file:` / `@env:` argument-source syntax that must remain separate from compose template sources.

## Knowledge
<!-- MUST apply write-gate: "If this item were lost, would the next agent make a wrong decision?"
Yes → write it. No → skip.

Target reader: a cold-starting agent that can only see spec + design + tasks + this Knowledge.
Exclude: anything already covered by spec/design/tasks (no restating).
Include: rejected approaches with reasons, implicit constraints, user preferences, API/env traps, insights that shaped design choices.

Format: - [timestamp] [Type] content
Types: Decision | Constraint | Gotcha | Rejected | Insight
  Decision  = directional choice made (with rationale)
  Constraint = hard limit imposed externally or by user
  Gotcha     = trap invisible without reading code/docs
  Rejected   = approach considered and discarded (with why — prevents successor from re-trying)
  Insight    = finding that shaped understanding but is not itself a decision

Project-level discoveries → ALSO append to project.md Notes.
Obsolete items → mark [obsolete: timestamp], never silently delete. -->

- [2026-06-10T01:22+08:00] [Decision] User chose command name `compose` with no `composer` alias.
- [2026-06-10T01:22+08:00] [Decision] Rendered body must never be embedded inside JSON output.
- [2026-06-10T01:22+08:00] [Decision] Default compose output target is a persistent file under `%TEMP%/agent-temp`; `--stdout` is required for body-to-stdout pipeline mode.
- [2026-06-10T01:22+08:00] [Decision] `exec:` is included in MVP but disabled unless `--allow-exec` is passed.
- [2026-06-10T01:22+08:00] [Decision] `--manifest` and temp cleanup are excluded from MVP.
- [2026-06-10T01:22+08:00] [Decision] Template/source input decoding should be宽松 for UTF/GBK Chinese-English environments; output files must be UTF-8 without BOM.
- [2026-06-10T01:22+08:00] [Decision] Multiline interpolation blocks are allowed; meaningful multiline command bodies must use JSON string escapes.
- [2026-06-10T01:22+08:00] [Gotcha] Compose template `${{stdin:}}` must remain separate from ASQ argument-source `@stdin`; using stdin for template loading would conflict with stdin as render input.
- [2026-06-10T01:33+08:00] [Decision] Syntax must distinguish command roles explicitly: source command first, then runtime controls, stream selectors, text transforms, and failure policies with deterministic normalization.
- [2026-06-10T01:33+08:00] [Decision] No-argument commands may omit the colon; body-taking commands still require `name: body`.
- [2026-06-10T01:33+08:00] [Decision] Add `compose --prompt` as an embedded agent-facing long guide, following `patch-edit --prompt` precedent.
- [2026-06-10T15:12+08:00] [Decision] Exec output that exceeds `--max-command-bytes` should not kill the child for size alone; drain stdout/stderr continuously, keep the rendered prefix, and spill excess output under a shared per-run 128MiB temp artifact budget.
- [2026-06-10T15:12+08:00] [Decision] `--total-timeout` means total render-phase wall-clock budget across all interpolations; an exec source uses the smaller of its local timeout and remaining total budget.
- [2026-06-10T15:12+08:00] [Decision] Compose JSON envelopes should include `meta.schemaVersion = 1`, `meta.cwd`, and optional spill `artifacts` in success/error payloads.

## Milestones
<!-- MUST append one line per session. Pure facts; new entries appended at the end.
CLI treats the last valid bullet as the latest milestone.
- [ISO timestamp] one-sentence summary -->

- [2026-06-10T01:22+08:00] Created change and drafted design from clarify decisions.
- [2026-06-10T01:33+08:00] Revised design grammar to separate source, runtime, stream, transform, and policy command roles.
- [2026-06-10T01:33+08:00] Added `--prompt` to the compose design.
- [2026-06-10T01:46+08:00] Drafted file-level implementation plan across six phases.
- [2026-06-10T02:10+08:00] Completed implementation and moved change to REVIEW.
- [2026-06-10T02:36+08:00] Created revision 001 for compile/render phase separation.
- [2026-06-10T02:52+08:00] Completed revision 001 refactor and returned change to REVIEW.
- [2026-06-10T15:12+08:00] Created revision 002 and returned the change to DOING for exec spill artifacts and JSON schema work.
- [2026-06-10T15:31+08:00] Completed revision 002 and returned the change to REVIEW after `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings` passed; `cargo fmt --check` remains blocked by pre-existing `src/builtins/imgweb/mod.rs` formatting diff.
- [2026-06-10T16:22+08:00] Addressed subagent review findings for spill marker preservation, full artifact reporting on truncation errors, and low-cost post-eval total-timeout checks; `cargo test` and clippy passed.
- [2026-06-10T16:47+08:00] Added compile-time source/stream compatibility validation so `stdout`/`stderr` on non-`exec` sources fail during `--check`; `cargo test` and clippy passed.
- [2026-06-10T16:58+08:00] Created and registered `.sspec/spec-docs/compose-template-engine.md` for the compose maintainer contract.
