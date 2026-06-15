# Memory: smart-indent

**Updated**: 2026-06-15T01:05+08:00

## Git Baseline (Immutable)
<!-- Captured during `sspec change new` before any change files are written.
This section records the change starting point in git and MUST NOT be edited or refreshed later. -->

- Captured: before change file creation
- Repository: `H:/SrcCode/playground/agent-squire`
- Branch: `feat/patch-with-indent`
- HEAD: `08a57d65b7eb943a2b4400d85dddd5d2ab48e126`
- Worktree: `dirty`
- Status Snapshot: raw `git status --short --branch` output

```text
## feat/patch-with-indent
 M src/builtins/patch_edit/model.rs
```

## State
<!-- Where we are and what's next — one to three lines.
This is the resume entry point; the first section an agent reads on cold start. -->

Implementation complete and change is in REVIEW. Smart-indent now uses block base-indent migration with strict default diagnostics, options API, ambiguity rejection, REPLACE migration safety, and smart idempotency.

## Key Files
<!-- Files critical to understanding/continuing this change.
- `path/file` — what it contains, why it matters -->

- `.sspec/changes/26-06-14T22-54_smart-indent/spec.md` — rewritten user-visible problem, scope, and compatibility contract.
- `.sspec/changes/26-06-14T22-54_smart-indent/design.md` — rewritten smart-indent matching/migration algorithm and API contract.
- `.sspec/changes/26-06-14T22-54_smart-indent/revisions/001-redefine-smart-indent.md` — records why spec/design were rewritten despite normal post-plan immutability.
- `src/builtins/patch_edit/match_apply.rs` — smart-indent candidate detection, strict diagnostics, apply path, and idempotency checks.
- `src/builtins/patch_edit/text.rs` — base-indent and indent migration helpers.
- `tests/patch_edit_compat.rs` — behavior tests for old API compatibility and smart-indent edge cases.

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

- [2026-06-15T00:37+08:00] Decision: Smart-indent means migrating a whole block between base indentation levels, not only adding target-side indentation and not only stripping SEARCH indentation.
- [2026-06-15T00:37+08:00] Constraint: Default mode remains strict for writes; diagnostic probing is allowed so users can distinguish indent mismatch from true content mismatch.
- [2026-06-15T00:37+08:00] Decision: More than one smart-indent candidate is always ambiguous, even if candidates share the same indent migration.
- [2026-06-15T00:37+08:00] Decision: `--smart-indent` must be idempotent by checking adjusted REPLACE content for already-applied state.
- [2026-06-15T00:37+08:00] Decision: Preserve old public Rust API and add options-based API for smart-indent.
- [2026-06-15T00:37+08:00] Rejected: Single `indent_delta` metadata is insufficient because the operation can be from longer to shorter indent or across different literal prefixes; use `indent_from` and `indent_to`.
- [2026-06-15T01:05+08:00] Gotcha: `cargo fmt` wants to reformat unrelated `src/builtins/read_range.rs`; that diff was reverted to keep this change scoped. `cargo fmt --check` remains blocked by that pre-existing formatting issue.
- [2026-06-15T01:05+08:00] Gotcha: Full `cargo test` is blocked by unrelated `tests/read_lines.rs` expectations for old compact output; targeted patch-edit tests pass.

## Milestones
<!-- MUST append one line per session. Pure facts; new entries appended at the end.
CLI treats the last valid bullet as the latest milestone.
- [ISO timestamp] one-sentence summary -->

- [2026-06-15T00:37+08:00] Rewrote smart-indent spec/design after clarify; next step is user alignment, then planning.
- [2026-06-15T01:05+08:00] Implemented smart-indent rewrite and moved change to REVIEW; targeted tests and clippy pass, full validation has unrelated read-range blockers.
