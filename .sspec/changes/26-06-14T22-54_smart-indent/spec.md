---
name: smart-indent
status: REVIEW
change-type: single
created: 2026-06-14T22:54:10
reference:
  - source: ".sspec/changes/26-06-14T22-54_smart-indent/revisions/001-redefine-smart-indent.md"
    type: "revision"
    note: "Redefine smart-indent as block base-indent migration."
---

# smart-indent

## Problem Statement

`asq patch-edit` fails with `search_not_found` when a SEARCH block has the correct relative content but a different base indentation level than the target file. This is common for agent-written patches in deeply nested formats such as YAML or nested code blocks, where counting exact leading spaces is error-prone.

The current failure gives no actionable distinction between "content is wrong" and "content matches after shifting the whole block's indent level".

## Proposed Solution

### Approach

Add a smart-indent matching stage after existing exact/loose matching. Smart-indent treats SEARCH and each candidate target window as blocks with their own base indent:

```text
indent_from = common leading whitespace prefix of SEARCH non-empty lines
indent_to   = common leading whitespace prefix of TARGET candidate non-empty lines

strip(indent_from, SEARCH) == strip(indent_to, TARGET candidate)
```

Default mode remains strict for writes: it does not apply smart-indent automatically. It only performs a diagnostic probe so users get `indent_mismatch` or `search_indent_ambiguous` instead of a generic `search_not_found`. With `--smart-indent`, exactly one smart-indent candidate is applied by migrating REPLACE from `indent_from` to `indent_to`.

This design supports both adding and removing base indentation; it does not assume `indent_to` is longer than `indent_from`.

### Key Change

- **Feat: smart-indent candidate detection** — add a matching stage that compares SEARCH and target windows after stripping each side's common non-empty-line indent.
- **Feat: strict-by-default diagnostic** — without `--smart-indent`, unique smart-indent match returns `indent_mismatch`; multiple smart-indent matches return `search_indent_ambiguous`; no smart match preserves `search_not_found`.
- **Feat: `--smart-indent` apply mode** — with `--smart-indent`, exactly one smart-indent candidate applies the patch using `match_mode: "indent_shift"`.
- **Feat: REPLACE indent migration** — migrate every non-empty REPLACE line from `indent_from` to `indent_to`; reject incompatible REPLACE lines with `replace_indent_incompatible`.
- **Feat: smart-indent idempotency** — with `--smart-indent`, detect already-applied state using the adjusted REPLACE content.
- **Feat: structured indent metadata** — expose `indent_from` and `indent_to` in match/apply results instead of a single `indent_delta` string.
- **Compat: options-based Rust API** — preserve the existing public `apply_patches(patch_text, project_root, dry_run)` API and add an options-based entrypoint for smart-indent.

### Scope Summary

| File | Change |
|------|--------|
| `src/builtins/patch_edit/model.rs` | Add options/result metadata for `indent_from` and `indent_to`. |
| `src/builtins/patch_edit/text.rs` | Add common-indent, strip-base-indent, and migrate-indent helpers. |
| `src/builtins/patch_edit/match_apply.rs` | Add smart-indent candidate search, ambiguity handling, adjusted REPLACE apply, and adjusted already-applied checks. |
| `src/builtins/patch_edit/mod.rs` | Add `--smart-indent`; route CLI through options API while preserving old API. |
| `src/builtins/patch_edit/output.rs` | Ensure compact/JSON output surfaces new statuses and indent metadata clearly. |
| `tests/patch_edit_compat.rs` | Replace prior smart-indent tests with coverage for base-indent migration, ambiguity, empty lines, idempotency, and API compatibility. |

### Design Reference

→ See [design.md](./design.md)
