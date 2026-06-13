---
revision: 2
date: 2026-06-13T19:45:00
trigger: "review-feedback"
---

# editable-fzf-selection

## Reason

Interactive fzf selection currently adds selected paths immediately. This makes `file:` line ranges cumbersome because users cannot edit the selected `file:path` into `file:path:start-end` before the source is added.

## Changes

### Spec Impact

FZF selector results MUST pass through an editable `edit>` confirmation step before being added:

```text
gather> file:
# fzf select src/main.rs
edit> file:src/main.rs
# user may press Enter to accept or edit to:
edit> file:src/main.rs:10-20
```

For multi-select, each selected candidate gets an edit step:

```text
edit 1/3> file:src/a.rs
edit 2/3> file:src/b.rs
edit 3/3> file:src/c.rs
```

Empty edited input skips that selected candidate. Invalid edited input reports an error and does not add that candidate.

### Design Impact

- Add a line editor dependency to support editable prefilled prompt input.
- `interactive.rs` SHOULD isolate the line-editor surface so non-interactive parsing/tests remain simple.
- FZF selectors MUST generate default source lines, then parse the edited line through existing `parse_source` so ranges and other future syntax reuse one parser.
- `glob:` fzf selection remains a grouped selected glob by default, but the edited lines MAY be converted to individual explicit sources if the user edits the prefix/body.

### Task Impact

Add revision tasks for line editor dependency, `edit>` confirmation, tests, docs, and verification.
