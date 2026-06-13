# Memory: gather

**Updated**: 2026-06-13T18:50+08:00

## Git Baseline (Immutable)
<!-- Captured during `sspec change new` before any change files are written.
This section records the change starting point in git and MUST NOT be edited or refreshed later. -->

- Captured: before change file creation
- Repository: `H:/SrcCode/playground/agent-squire`
- Branch: `main`
- HEAD: `6760a37ca116820595fc7be6625dd0e717cdf76b`
- Worktree: `clean`
- Status Snapshot: raw `git status --short --branch` output

```text
## main...origin/main [ahead 1]
```

## State
<!-- Where we are and what's next — one to three lines.
This is the resume entry point; the first section an agent reads on cold start. -->

Design gaps from the pre-planning audit have been resolved in `spec.md`/`design.md`; interactive mode now retains fzf selection via selector lines. Next: user confirms the updated design, then fill `tasks.md`.

## Key Files
<!-- Files critical to understanding/continuing this change.
- `path/file` — what it contains, why it matters -->

- `.sspec/changes/26-06-13T18-06_gather/spec.md` — user-visible contract and scope for `gather`.
- `.sspec/changes/26-06-13T18-06_gather/design.md` — current design draft; must be revised before task planning.
- `.sspec/spec-docs/compose-template-engine.md` — compose contracts that `gather` depends on, especially file ranges, exec trust boundary, and output behavior.
- `src/builtins/compose/mod.rs` — current compose CLI API; `gather` integration choices must match this implementation.

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

- [2026-06-13T18:32+08:00] [Insight] Pre-planning audit found unresolved design gaps: `dir`/`glob` content semantics, compose file-range syntax mismatch, exec enablement/trust boundary, gather-vs-compose output identity/temp prefix, named-flag ordering, interactive Tab/fzf implementation model, and template-body quoting/injection safety.
- [2026-06-13T18:32+08:00] [Gotcha] Compose file ranges use `${{file: PATH |> lines: START-END}}`; `${{file: PATH:START-END}}` is treated as a file path, not a line slice.
- [2026-06-13T18:32+08:00] [Gotcha] Compose `exec:` sources fail unless `allow_exec` is enabled. Any `gather` design that renders `cmd:`, `tree:`, `dir:`, or `glob:` via `${{exec: ...}}` must explicitly decide whether `gather` enables exec implicitly for those explicit source types.
- [2026-06-13T18:32+08:00] [Gotcha] Delegating directly to `compose::run` will report command identity and default temp paths as `compose`/`asq-compose-*`, while the current `gather` design promises `gather`/`asq-gather-*` behavior.
- [2026-06-13T18:45+08:00] [Decision] `dir:` and `glob:` render grouped containers with a manifest and nested `FILE` content blocks for matched files. Empty matches render an empty container with `Matched files: (none)`.
- [2026-06-13T18:45+08:00] [Decision] Interactive MVP is line-oriented: full `prefix:body` lines or prefix-only `prefix:` followed by one body line. Tab/fzf completion is outside MVP. [obsolete: 2026-06-13T18:50+08:00]
- [2026-06-13T18:45+08:00] [Decision] `gather` enables compose exec internally when generated templates contain command-backed sources such as `cmd:` or command-backed `tree:`.
- [2026-06-13T18:45+08:00] [Decision] Named flags do not guarantee cross-flag ordering; positional sources preserve written order.
- [2026-06-13T18:45+08:00] [Decision] Generated compose source bodies are JSON-string-quoted so user-provided paths/commands are data, not compose template syntax.
- [2026-06-13T18:50+08:00] [Decision] Interactive MVP keeps fzf selection without raw Tab handling: selector-only lines (`file:`, `dir:`, `tree:`, `glob:`) open fzf; explicit `prefix:body` remains supported.

## Milestones
<!-- MUST append one line per session. Pure facts; new entries appended at the end.
CLI treats the last valid bullet as the latest milestone.
- [ISO timestamp] one-sentence summary -->
- [2026-06-13T18:32+08:00] Reviewed handover/spec/design/tasks and identified design ambiguities that should be aligned before planning or implementation.
- [2026-06-13T18:45+08:00] Updated spec/design for directory/glob expansion, line-oriented interactive mode, exec enablement, source ordering, and compose template quoting.
- [2026-06-13T18:50+08:00] Restored fzf as part of interactive mode via Enter-triggered selector lines instead of Tab key capture.
