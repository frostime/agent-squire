---
name: rearrange-dst
status: REVIEW
change-type: single
created: 2026-06-29T19:41:26
reference:
  - source: .sspec/changes/26-06-29T19-41_rearrange-dst/reference/DST-SPEC.md
    type: doc
    note: Source behavior specification for the new Arrange state-transition DSL.
---

# rearrange-dst

## Problem Statement

The committed `asq rearrange` branch implementation models edits as one-file sequential actions (`move/copy/delete/rearrange`), causing the command to conflict with the newly accepted DSL goal: auditable whole-file `before -> after` state transitions across one or more files.

Current branch implementation must be replaced rather than revised: old implicit gap/prefix/suffix behavior is now invalid, and keeping it would make the new DSL harder to reason about and review.

## Proposed Solution

### Approach

Rewrite `asq rearrange` around the behavior defined in [reference/DST-SPEC.md](./reference/DST-SPEC.md). The CLI command name stays `rearrange`; the old v1 DSL and behavior are removed.

The new implementation uses a staged architecture: parse DSL AST → resolve path identity → read one pre-state snapshot → validate and bind materials → materialize target file states → render preview → optionally apply writes/deletes. The design separates syntax, path identity, material provenance, text I/O, and preview rendering so each DSL invariant has one owner.

### Behavior Contract

**BC-1 Command surface**
- Surface: `asq rearrange [SPEC]`, `--stdin`, `-f/--file`, `--dry-run`, `--yes`, `--prompt`, global `--cwd`, `--print`, `--json`.
- After: command name remains `rearrange`; `rearr` alias may remain as CLI compatibility alias.
- Boundary: old v1 DSL is not supported.

**BC-2 State-transition DSL**
- `share` blocks declare read-only named materials.
- `arrange` blocks declare one target file's complete `before` and complete `after` file state.
- `before` supports `<missing>`, `<empty>`, and sequence items: anonymous range, named range, explicit gap.
- `after` supports `<missing>`, `<empty>`, and sequence references to local materials, local gaps, anonymous before ranges, `share` material, or other slugged arrange before named chunks.
- Multiple `arrange` blocks may participate in one pre-state snapshot; there is no execution order.

**BC-3 Validation and path identity**
- Duplicate slugs, duplicate normalized paths, share/arrange same path, hidden gaps, invalid coverage, invalid ranges, unknown references, duplicate local names, and invalid state transitions fail before any write.
- Path identity is stricter than patch-edit because it is a DSL invariant, not just an I/O target: existing paths use canonical identity; missing arrange targets use nearest existing ancestor plus normalized suffix; paths may not escape `--cwd`.
- Creating a missing target may create parent directories.
- Path prefix conflicts inside the same spec fail to avoid implicit directory state transitions.

**BC-4 Text and write behavior**
- Existing target rewrites preserve target newline style, final-newline state, and encoding when possible.
- New files use UTF-8, LF, and final newline for non-empty sequence output; `<empty>` writes 0 bytes.
- Non-encodable output for an existing non-UTF target fails instead of silently replacing characters.
- `--dry-run` or no `--yes` performs no writes.

**BC-5 Preview / JSON**
- Compact preview summarizes shares, target before/after states, exported materials, explicit gaps, and derived target effects in a predictable, reviewable form.
- JSON output uses the existing `Envelope<T>` shape with `command: "rearrange"`, structured success data, and `meta.error_code` on failure.

### Implementation Changes

- **refactor(rearrange): Replace v1 action model with state-transition model** — rewrite core AST/error types and remove old `Action` semantics. Serves BC-1/BC-2.
- **feat(parser): Parse share/arrange DST DSL** — implement line-role tokenization plus block/payload parsing, including raw range retention. Serves BC-2/BC-3.
- **feat(path): Add rearrange path identity resolver** — normalize paths, enforce workspace containment, duplicate identity, missing-target parents, and prefix conflict checks. Serves BC-3.
- **feat(plan): Validate snapshot and material provenance** — read pre-state once, validate file states/gaps/coverage/references, build material registry, materialize after states. Serves BC-2/BC-3/BC-4.
- **feat(textio): Support create/delete/empty and encoding-safe writes** — adapt text I/O to new state outputs and parent directory creation. Serves BC-4.
- **feat(output): Render DST preview and JSON** — replace action/diff preview with state/provenance/effect summary. Serves BC-5.
- **test(rearrange): Replace v1 coverage with DST behavior tests** — update integration and unit tests for new DSL semantics. Serves BC-1..BC-5.
- **docs(prompt): Replace prompt guide with DST DSL guide** — make `A-end` the recommended EOF style while allowing numeric EOF guards. Serves BC-1/BC-2.

### Scope Summary

| File | Change | Effort |
|------|--------|--------|
| `src/builtins/rearrange/mod.rs` | Keep CLI surface, route new pipeline | S |
| `src/builtins/rearrange/error.rs` | New structured error module | S |
| `src/builtins/rearrange/ast.rs` | New parsed/state/material vocabulary | M |
| `src/builtins/rearrange/parser.rs` | Rewrite parser for DST DSL | L |
| `src/builtins/rearrange/path.rs` | New path identity resolver | M |
| `src/builtins/rearrange/plan.rs` | Rewrite snapshot/validation/materialization | L |
| `src/builtins/rearrange/textio.rs` | Extend text file render/write/delete/create | M |
| `src/builtins/rearrange/output.rs` | Rewrite compact/json preview | M |
| `src/builtins/rearrange/prompt.md` | Replace old DSL guide | S |
| `tests/rearrange.rs` | Replace old v1 integration tests | L |

### Design Reference

See [design.md](./design.md).
