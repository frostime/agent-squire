---
name: extarct-markdown-link
status: DONE
change-type: single
created: 2026-06-01 15:46:16
reference:
- source: .sspec/requests/26-06-01T15-32_extarct-markdown-link.md
  type: request
  note: Linked from request
---
<!-- MUST follow frontmatter schema:
status: PLANNING | DOING | REVIEW | DONE | BLOCKED
change-type: single | sub
reference?: Array<{source, type: 'request'|'root-change'|'sub-change'|'prev-change'|'doc'|'revision', note?}>
-->

# extarct-markdown-link

## Problem Statement

Markdown reference discovery currently requires ad-hoc search/parsing, causing agents to miss or misclassify links when building a file-based reference graph across Markdown documents.

## Proposed Solution

### Approach

Add a built-in `md-links` CLI, shaped like `md-toc`: it accepts one or more Markdown files/directories/globs, recursively expands directories to `.md` files, parses supported Markdown-like reference syntaxes, resolves file references against the source file directory and a configurable workspace, and reports existence.

The command will stay dependency-light and deterministic: implement a scanner suitable for agent navigation rather than a full CommonMark parser. It will skip fenced code blocks, preserve line numbers, and support JSON envelope output for graph consumers.

### Key Change

**Feat A: Markdown link extraction command**
- Add `squire md-links` with alias `mdlinks`.
- Inputs mirror `md-toc`: files, directories, globs; default source `.`.
- Add `--workspace DIR`, defaulting to effective `ctx.cwd` after global `--cwd` is applied.

**Feat B: Link pattern coverage**
- Markdown links/images: `[text](target)`, `![alt](target)`.
- Wiki/block references: `[[target]]`, `(target)` when written as double-paren block reference `((target))`.
- Inline code path references: `` `target` `` when target looks like URL/path.
- Angle references: `<target>` when target looks like URL/path.
- URL targets: `http://...`, `https://...`.
- File targets: absolute paths, `./`/`../` relative paths, workspace-relative paths (`src/a.md`, `/src/a.md`).

**Feat C: File resolution + graph-ready output**
- For each link, emit source file, line number, syntax kind, raw target, target class (`url`/`file`/`unknown`), normalized/resolved target path when applicable, and `exists` for file targets.
- Strip fragment/query suffixes only for file-existence checks while preserving the raw target.
- Wiki links without an extension resolve with an additional `.md` fallback.

**Test D: CLI behavior tests**
- Cover source expansion, representative syntaxes, workspace-relative resolution, missing files, JSON envelope shape, and fenced-code skipping.

### Scope Summary

| File | Change |
|---|---|
| `src/cli.rs` | Register `md-links` subcommand + alias and route execution. |
| `src/builtins/mod.rs` | Export new builtin module. |
| `src/builtins/md_links/` | Implement the command as a split vertical module: args/run, models, source expansion, parser, resolver, output. |
| `tests/md_links.rs` | Add behavior-oriented integration coverage for command behavior. |
| `CHANGELOG.md` | Record the new built-in CLI feature. |

### Design Reference

→ See [design.md](./design.md)
