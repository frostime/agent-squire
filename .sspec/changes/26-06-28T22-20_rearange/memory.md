# Memory: rearange

**Updated**: 2026-06-28T22:50

## Git Baseline (Immutable)
<!-- Captured during `sspec change new` before any change files are written.
This section records the change starting point in git and MUST NOT be edited or refreshed later. -->

- Captured: before change file creation
- Repository: `H:/SrcCode/playground/agent-squire`
- Branch: `feat/rearange`
- HEAD: `d848daf7c1e3d3fc9b23d6287448630e553543b8`
- Worktree: `dirty`
- Status Snapshot: raw `git status --short --branch` output

```text
## feat/rearange
A  .sspec/requests/26-06-28T18-48_rearange.md
```

## State

Implementation complete; all 4 phases done. `cargo test` 157 pass, clippy clean, fmt clean. Ready for Review/user acceptance.

## Key Files

- `src/builtins/rearrange/plan.rs` — planner core (resolve/validate/materialize); original-snapshot coordinates; `splice` shared by move/copy/delete; `plan_rearrange` slot model
- `src/builtins/rearrange/textio.rs` — sole owner of newline/encoding knowledge; planner only sees `Vec<String>`
- `src/builtins/rearrange/parser.rs` — DSL parse, single-action enforcement
- `tests/rearrange.rs` — RFC case 1-5 + CRLF + multi-action + FILE_NOT_FOUND + JSON

## Knowledge

- [2026-06-28] [Decision] gap=slot is the default and == user's "广义 swap": physical slots fixed by line order, slot contents permuted, undeclared gaps pinned between slots. Confirmed correct; gap=error rejected as default (would reject most real swaps with blank lines between blocks).
- [2026-06-28] [Constraint] v1 scope locked by user: single-file, exactly one action per call, DSL-only, bare numeric ranges (no `L` prefix, aligns with read-range), no backup/journal/rollback, no FILE_CHANGED snapshot. JSON input + cross-file deferred to v2.
- [2026-06-28] [Gotcha] rearrange gaps must be POSITIONAL: gaps[i] sits between slot i and slot i+1 even when empty. Filtering empty gaps out of the vec misaligns the index (produced `B,h1,D,h2,C,A` instead of `B,h1,D,C,h2,A`). Empty placeholder `(0,0,vec![])` kept; `non_empty_gaps` filters only for reporting.
- [2026-06-28] [Gotcha] textio self-contained, does NOT reuse `patch_edit/io.rs` (kept surgical; ~40 lines encoding-detect duplicated by design choice #4).
- [2026-06-28] [Decision] move anchor on block boundary (before start / after end) = no-op, allowed; only strictly-interior anchor → ANCHOR_INSIDE_MOVED_CHUNK.
- [2026-06-28] [Gotcha] bash `./target/debug/squire.exe` on this machine resolves to a STALE installed squire on PATH (`/g/Enviroment/Rust/.cargo/bin`). Verify the freshly-built binary via `cmd.exe //c "target\debug\squire.exe ..."` instead.
- [2026-06-28] [rev-001] Review (deepseek-v4-pro xhigh) found 4 conformance defects + 5 DSL ambiguities. Fixed: JSON `data` now carries `chunks`+`action` (BC-5); `gap=error`→new `NON_EMPTY_GAP` code; duplicate rearrange names→`REARRANGE_SET_MISMATCH`; `--dry-run` overrides `--yes` (`write = yes && !dry_run`); `--yes` no-op labeled `(no-op)`; chunk names constrained to identifier `[A-Za-z_][A-Za-z0-9_]*` + reserved keywords (disambiguates region-vs-name and ` to ` separator). DSL formalized as EBNF in design §2.1 + prompt.md.

## Milestones

- [2026-06-28T22:50] rearrange v1 implemented across 4 phases; tests/clippy/fmt green; live gap=slot output matches design Mock C.
- [2026-06-28T23:20] rev-001 applied: 4 review fixes + DSL EBNF formalization; 16 integration tests green, clippy/fmt clean; 3 rev User Checks verified live via `cargo s`.
