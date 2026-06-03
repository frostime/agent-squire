---
revision: 1
date: 2026-06-03T21:23:57
trigger: "review-feedback"
---

# use cwd as backlinks path root

## Reason

Review feedback identified that `md-backlinks` introduced two path coordinate systems: global `--cwd` and command-local `--workspace`. That makes agent-facing use harder to reason about and caused an implementation ambiguity where focus pages were normalized against `--workspace` while corpus discovery still followed process CWD. The revised design uses effective CWD as the only path root for the new backlinks command.

## Changes

### Spec Impact

Logical changes to `spec.md`:

- `md-backlinks` no longer has a `--workspace` option.
- `md-backlinks <pages...>` focus pages are resolved relative to effective CWD.
- `md-backlinks --from <corpus...>` corpus paths/globs are resolved relative to effective CWD.
- JSON metadata should report the effective CWD/root instead of command-local workspace.
- Existing `md-links --workspace` remains unchanged for backwards compatibility; this revision only changes the new `md-backlinks` command before release.

### Design Impact

Logical changes to `design.md`:

```text
old:
  workspace = args.workspace.unwrap_or(ctx.cwd)
  focus_set = normalize_focus_pages(args.pages, workspace)
  corpus_files = discover_backlink_corpus(args.from_or_dot, workspace, policy)
  resolve_link(raw, source_file, workspace)

new:
  root = ctx.cwd
  focus_set = normalize_focus_pages(args.pages, root)
  corpus_files = discover_backlink_corpus(args.from_or_dot, root, policy)
  resolve_link(raw, source_file, root)
```

CLI contract becomes:

```bash
asq --cwd <root> md-backlinks <pages...> [--from <corpus>...] [--no-gitignore]
```

### Task Impact

Add feedback tasks to `tasks.md`:

- Remove `--workspace` from `src/builtins/md_backlinks/mod.rs`.
- Make focus normalization, corpus discovery, and link resolution use `ctx.cwd` as the single root.
- Update JSON metadata from `workspace` to `cwd` or `root`.
- Update tests to cover global `--cwd` root semantics and absence of `--workspace` on `md-backlinks`.
