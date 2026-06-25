# Memory: img-clipboard

**Updated**: 2026-06-25T18:06+08:00

## Git Baseline (Immutable)
<!-- Captured during `sspec change new` before any change files are written.
This section records the change starting point in git and MUST NOT be edited or refreshed later. -->

- Captured: before change file creation
- Repository: `H:/SrcCode/playground/agent-squire`
- Branch: `main`
- HEAD: `70fbf69b77e46056607a4dc7f9aa5fb61ee17442`
- Worktree: `clean`
- Status Snapshot: raw `git status --short --branch` output

```text
## main...origin/main
```

## State

Review feedback fix complete and status is REVIEW. Next: user retry `cargo s img` / `asq img` with the original screenshot-tool clipboard image and report result.

## Key Files

- None beyond `spec.md` Scope Summary.

## Knowledge

- [2026-06-25T17:02+08:00] [Insight] Clarify grep found clipboard support only in `src/builtins/imgweb/web/index.html` browser JS; Rust CLI had no clipboard dependency/API before this change.
- [2026-06-25T17:02+08:00] [Decision] User chose: add `asq img`, keep `asq imgweb`, default `asq img` to read clipboard image, compact output as local path, JSON output as path/uri/size/mime, and first version read-only image-only.
- [2026-06-25T17:02+08:00] [Rejected] Platform shell clipboard commands were rejected for design because they would make cross-platform behavior and errors less predictable than Rust dependencies.
- [2026-06-25T17:08+08:00] [Decision] User confirmed `imgweb` should be hidden as a legacy entrypoint while remaining executable for backwards compatibility.
- [2026-06-25T17:51+08:00] [Constraint] Agent did not run direct `asq img` against the live clipboard because doing so would read and persist the user's current clipboard image; user review should perform this black-box check intentionally.
- [2026-06-25T18:06+08:00] [Insight] User's failing clipboard image exposed `PixPinData`, `DeviceIndependentBitmap` (`CF_DIB`), and `Format17` (`CF_DIBV5`); `arboard` failed on conversion, so Windows fallback must try `CF_DIB` directly.

## Milestones

- [2026-06-25T17:02+08:00] Clarify+Design: confirmed clipboard/web CLI direction and drafted `img-clipboard` spec/design.
- [2026-06-25T17:08+08:00] Design: revised visibility so public entrypoint is `img`; `imgweb` remains hidden legacy.
- [2026-06-25T17:51+08:00] Implement: completed code/docs, passed quality gates, moved change to REVIEW.
- [2026-06-25T18:06+08:00] Review feedback: recorded revision 001, added Windows DIB fallback, verified generated clipboard bitmap saves through `asq img`, and returned to REVIEW.
