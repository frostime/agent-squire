---
revision: 1
date: 2026-06-15T00:37:36+08:00
trigger: "correction"
---

# redefine-smart-indent

## Reason

Review and clarify found that the previous spec/design and implementation described different algorithms. The earlier design said to compute a common indent prefix from SEARCH and strip it, while the implementation/tests treated smart-indent as adding a target-side outer indent. The clarified user intent is broader: smart-indent should treat the whole SEARCH/REPLACE block as movable across indentation levels, including both increasing and decreasing the base indent, while preserving internal relative indentation.

User explicitly allowed rewriting `spec.md` and `design.md` despite the normal post-plan revision convention, because the current version is not trustworthy enough as a prediction contract.

## Changes

### Spec Impact

Rewrite the problem and solution around block base-indent migration:

- Smart-indent candidate detection compares SEARCH and target windows after stripping each side's own common non-empty-line indent.
- `indent_from` is SEARCH common indent; `indent_to` is target-window common indent.
- Default mode remains strict for writes, but performs smart-indent diagnostic probing.
- `--smart-indent` applies only when there is exactly one candidate.
- More than one candidate is `search_indent_ambiguous`, regardless of whether the candidates use the same indent migration.
- REPLACE migration performs `indent_from -> indent_to` on every non-empty line and rejects incompatible REPLACE lines.
- `--smart-indent` must be idempotent by checking the adjusted REPLACE content for `already_applied`.
- Public API keeps the old `apply_patches(...)` entrypoint and adds options-based API for smart-indent.

### Design Impact

Replace the previous `compute_common_indent_prefix(SEARCH) then strip SEARCH` algorithm with a candidate-window algorithm:

```text
for each target window with len == search_lines.len():
  search_from = common_indent(SEARCH non-empty lines)
  target_to   = common_indent(TARGET_WINDOW non-empty lines)
  if strip_prefix(search_from, SEARCH non-empty lines)
     == strip_prefix(target_to, TARGET_WINDOW non-empty lines):
       candidate(indent_from=search_from, indent_to=target_to)
```

### Task Impact

Discard previous implementation tasks and re-plan after design alignment. Existing code should be treated as disposable unless it matches the rewritten design.