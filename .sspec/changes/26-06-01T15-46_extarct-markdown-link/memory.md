# Memory: extarct-markdown-link

**Updated**: 2026-06-01T17:13+08:00

## Git Baseline (Immutable)
<!-- Captured during `sspec change new` before any change files are written.
This section records the change starting point in git and MUST NOT be edited or refreshed later. -->

- Captured: before change file creation
- Repository: `H:/SrcCode/playground/agent-squire`
- Branch: `main`
- HEAD: `2fab3eb678dcd8cf77f1b5ae0fff304f4669f80a`
- Worktree: `dirty`
- Status Snapshot: raw `git status --short --branch` output

```text
## main...origin/main [ahead 1]
A  .sspec/requests/26-06-01T15-32_extarct-markdown-link.md
```

## State

User accepted the CLI; change and request marked DONE. Next: squash merge `wip/extarct-markdown-link` into `main` and delete the WIP branch.

## Key Files

- `.sspec/requests/26-06-01T15-32_extarct-markdown-link.md` — source request and lifecycle instruction.
- `.sspec/changes/26-06-01T15-46_extarct-markdown-link/spec.md` — change scope and user-visible feature definition.
- `.sspec/changes/26-06-01T15-46_extarct-markdown-link/design.md` — UX/algorithm design for `md-links`.

## Knowledge

- [2026-06-01T15:56+08:00] [Decision] User wants design biased toward UX and algorithm behavior; internal Rust implementation details are secondary.
- [2026-06-01T15:56+08:00] [Decision] `((...))` is explicitly SiYuan block-ref syntax, not generic link syntax. Recognize `((20260531010806-35bkoxa '2026-05-31'))` / double-quoted variant; support `siyuan://` as URL scheme.
- [2026-06-01T15:56+08:00] [Decision] Path resolution should support both `/` and `\\`, normalize display to `/`, and for `/src`-style targets try workspace-relative first before OS-root fallback.
- [2026-06-01T16:02+08:00] [Decision] Compact output should use dense fields with JSON-escaped strings, not padded columns; `--print json` remains the robust machine interface. [updated: 2026-06-01T16:06+08:00] Group by source file to avoid repeating long paths and wasting tokens.
- [2026-06-01T16:02+08:00] [Decision] Implement `md_links` as a split vertical module because parsing + resolution + output is expected to exceed comfortable single-file complexity; tests remain behavior-oriented per `write-good-tests`.
- [2026-06-01T16:07+08:00] [Decision] User requested WIP branch workflow; created `wip/extarct-markdown-link` and committed current SSPEC design as `0b153f2` before Plan.
- [2026-06-01T16:41+08:00] [Gotcha] `cargo clippy --all-targets --all-features -- -D warnings` fails on unrelated pre-existing lints in `imgweb`, `info`, `patch_edit`, `tree`, and `external`; `md_links` had one clippy lint and it was fixed.

## Milestones

- [2026-06-01T15:56+08:00] Clarified requirements and revised design toward graph-building UX, workspace-aware path resolution, and SiYuan-specific references.
- [2026-06-01T16:02+08:00] Revised design after feedback: compact output is dense/parseable, implementation is split into focused files, and test strategy references `write-good-tests`.
- [2026-06-01T16:06+08:00] Revised compact output again: group by source file and use dense per-link records inside each group.
- [2026-06-01T16:07+08:00] Created WIP branch checkpoint commit and wrote phase-based implementation plan in `tasks.md`.
- [2026-06-01T16:41+08:00] Implemented `md-links`, added behavior tests, updated changelog, and moved change status to REVIEW.
- [2026-06-01T17:13+08:00] User accepted `md-links`; marked change and request DONE before squash merge.
