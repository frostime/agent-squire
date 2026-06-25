# Memory: data-toc

**Updated**: 2026-06-26T00:53+08:00

## Git Baseline (Immutable)
<!-- Captured during `sspec change new` before any change files are written.
This section records the change starting point in git and MUST NOT be edited or refreshed later. -->

- Captured: before change file creation
- Repository: `H:/SrcCode/playground/agent-squire`
- Branch: `feat/data-toc`
- HEAD: `747432de944f7a14627e6c93114b9eb6eee57c9e`
- Worktree: `clean`
- Status Snapshot: raw `git status --short --branch` output

```text
## feat/data-toc
```

## State

Phase 1 JSON/JSONL MVP implementation is complete and validated. Phase 2 YAML via `yq` and Phase 3 heuristics/examples remain pending in `tasks.md`.

## Key Files

- `.sspec/changes/26-06-25T23-46_data-toc/spec.md` — behavior contract, implementation labels, scope summary.
- `.sspec/changes/26-06-25T23-46_data-toc/design.md` — CLI interface, data model, analysis flow, output previews.
- `.sspec/changes/26-06-25T23-46_data-toc/tasks.md` — phased execution plan; Phase 1 is the current implementation target.
- `.sspec/changes/26-06-25T23-46_data-toc/reference/gpt-prd.md` — archived source PRD draft from `.sspec/tmp/gpt-prd.md`.
- `src/cli.rs` — CLI subcommand registration and global output flags.
- `src/runtime/output.rs` — `Envelope<T>` JSON output convention.
- `src/builtins/` — built-in command module pattern.

## Knowledge

- [2026-06-25T23:48+08:00] [Decision] Use one SSPEC single change with internal Phase 1/2/3, not root/sub-changes. Phase 1 can deliver MVP, but the full target remains represented in the same change.
- [2026-06-25T23:48+08:00] [Decision] Phase 1 MVP includes `--prompt`; the prompt is required because existing agent-facing commands such as `compose`, `gather`, and `patch-edit` expose usage guides this way.
- [2026-06-25T23:48+08:00] [Decision] Primary command is `data-toc`; only alias is `datatoc`. Do not add `json-toc` or `jsontoc` unless a later review changes the public CLI surface.
- [2026-06-25T23:48+08:00] [Constraint] Work is on branch `feat/data-toc`; create WIP checkpoint commits when requested milestones are reached.
- [2026-06-25T23:48+08:00] [Rejected] Do not implement `data-toc` as an external mapped command; it needs built-in CLI integration, JSON envelope output, and internal aggregation logic.
- [2026-06-26T00:53+08:00] [Insight] Phase 1 validation passed with `cargo fmt`, `cargo test --test data_toc`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`.

## Milestones

- [2026-06-25T23:48+08:00] Clarify completed; created `feat/data-toc` and drafted design artifacts for a phased built-in `data-toc` command.
- [2026-06-26T00:19+08:00] Checkpoint commit `2a9d18e` recorded design artifacts; plan phase created Phase 1/2/3 task breakdown.
- [2026-06-26T00:53+08:00] Implemented Phase 1 `data-toc` JSON/JSONL MVP, updated docs/tests, and completed validation commands.
