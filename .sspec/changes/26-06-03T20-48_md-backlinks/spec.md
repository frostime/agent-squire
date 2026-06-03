---
name: md-backlinks
status: DONE
change-type: single
created: 2026-06-03T20:48:45
reference:
  - source: ".sspec/changes/26-06-03T20-48_md-backlinks/revisions/001-use-cwd-as-backlinks-path-root.md"
    type: "revision"
    note: "Use effective CWD as the single path root for md-backlinks."
---

# md-backlinks

## Problem Statement

0 built-in commands can answer “which Markdown files link to file X?”, causing agents and humans to reconstruct backlinks manually from forward-link output or ad-hoc text search. Text search is incomplete because backlinks are defined by Markdown link resolution, not raw filename occurrence.

## Proposed Solution

### Approach

Add a new `md-backlinks` builtin command. Keep `md-links` as the existing forward-link extractor with unchanged CLI and output. `md-backlinks` uses positional arguments as focus pages and `--from` as the corpus/search scope, so the mental model is symmetric: `md-links <pages...>` shows outgoing links from pages; `md-backlinks <pages...> --from <corpus...>` shows incoming links to pages from corpus.

Internally, `md-backlinks` reuses the existing Markdown link parser and resolver from `src/builtins/md_links/`. Backlinks are computed by building forward edges from corpus files, resolving each edge target, and grouping matching file-target edges by focus page. This avoids a separate backlink parser and keeps forward/backward results consistent.

Corpus discovery for backlinks uses `ignore::WalkBuilder` with `.gitignore` and built-in noise skips enabled by default. This makes backlink results auditable and avoids scanning generated/vendor directories unless the user explicitly opts out with `--no-gitignore`.

### Key Change

**Feat A: Backlinks CLI**
Add `md-backlinks` as a built-in command with syntax:

```bash
asq md-backlinks <pages...> [--from <path>...] [--workspace <dir>] [--no-gitignore]
```

**Feat B: Shared link graph core**
Expose a small `md_links` internal API that converts Markdown source files into resolved file-link edges. Both `md-links` and `md-backlinks` rely on the same parser/resolver semantics.

**Feat C: Backlink corpus discovery**
Discover Markdown corpus files from explicit files, directories, and globs. Directory walks respect `.gitignore`, global gitignore, `.git/info/exclude`, and built-in skip names by default; `--no-gitignore` disables those filters.

**Feat D: Backlink output**
Add compact and JSON output for target-grouped backlink results. JSON uses the standard `Envelope<T>` with command `md-backlinks`; compact output is dense and source-line oriented.

**Test E: TDD behavior matrix**
Add integration tests before/with implementation to lock resolution, false-positive rejection, corpus filtering, explicit ignored-file behavior, compact output, and `md-links` compatibility.

### Scope Summary

| File | Change |
|------|--------|
| `src/cli.rs` | Register `md-backlinks` command and dispatch to new builtin |
| `src/builtins/mod.rs` | Export new builtin module |
| `src/builtins/md_backlinks/mod.rs` | New command args, backlink computation, and output |
| `src/builtins/md_links/model.rs` | Make shared model types usable by backlink graph code; add edge structs if needed |
| `src/builtins/md_links/parse.rs` | Expose parser as `pub(crate)` if needed; preserve behavior |
| `src/builtins/md_links/resolve.rs` | Expose resolver as `pub(crate)` if needed; preserve behavior |
| `src/builtins/md_links/sources.rs` | Add/adjust shared source discovery helpers without changing `md-links` defaults |
| `src/builtins/md_links/graph.rs` | New shared resolved-edge builder if this keeps modules simpler |
| `tests/md_backlinks.rs` | New integration tests covering backlink behavior |
| `tests/md_links.rs` | Existing tests remain authoritative for forward compatibility |

What stays unchanged:

- Existing `md-links` CLI, aliases, JSON fields, compact output, and resolution behavior.
- Existing `file-tree` behavior.
- No external `rg` dependency or shell-out requirement.

### Design Reference

→ See [design.md](./design.md)
