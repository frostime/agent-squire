---
name: gather
status: REVIEW
change-type: single
created: 2026-06-13T18:06:22
reference:
  - source: ".sspec/tmp/26-06-13T17-30_clarify_files-to-prompt.md"
    type: "doc"
    note: "Clarify notes for gather command"
  - source: ".sspec/changes/26-06-13T18-06_gather/revisions/001-interactive-controls.md"
    type: "revision"
    note: "Interactive commands and ignored-file selection controls"
  - source: ".sspec/changes/26-06-13T18-06_gather/revisions/002-editable-fzf-selection.md"
    type: "revision"
    note: "Editable confirmation for fzf-selected sources"
---

# gather

## Problem Statement

Users need to assemble multiple files, code snippets, directory trees, and command outputs into a single prompt file for LLM interactions. Currently, this requires manual copy-paste or ad-hoc shell scripts, causing:
- **Time waste**: 5-10 minutes per prompt assembly
- **Error-prone**: Missing files, wrong order, inconsistent formatting
- **Non-reproducible**: Different formatting each time
- **Context switching**: Leaving the terminal to use VS Code extensions

## Proposed Solution

### Approach

Add a new built-in `gather` command that assembles context from multiple sources into a formatted prompt file. The command supports both CLI flags and interactive mode (with fzf integration).

**Key design decisions**:
1. **Based on compose**: `gather` generates a compose template, then delegates to `compose` for rendering. This reuses existing infrastructure and keeps implementation simple.
2. **Fixed output format**: Uses a custom delimiter format (`====== TYPE-START: path ======`) that's distinct from markdown to avoid confusion when the content itself is markdown.
3. **Interactive fzf mode**: Supports fzf-based file/directory selection without raw Tab key handling.
4. **Prefix syntax**: Uses `file:path`, `dir:path`, `tree:path`, `glob:pattern`, `cmd:command` syntax for interactive input.

### Key Change

**Feat A: Gather CLI** — Add `asq gather` / `squire gather` with file, directory, tree, glob, and command input options.

**Feat B: Interactive Mode** — Add `--interactive` / `-i` flag for interactive input. Users may enter a full source (`file:src/main.rs`) or a selector line (`file:`, `dir:`, `tree:`) that opens fzf. Tab key capture is outside MVP; fzf is triggered by Enter on selector lines.

**Feat C: Template Generation** — Generate compose template with custom delimiter format (`====== TYPE-START: path ======`), JSON-quote template bodies, and delegate rendering to compose internals.

**Feat D: Source Resolution** — Resolve file, directory, tree, glob, and command sources using `--cwd`,宽松 text decoding, binary refusal, directory/glob expansion into file content blocks, and explicit exec enablement for command-backed sources.

**Feat E: Output Contract** — Write rendered body to temp file by default, explicit output path with `-o`, explicit stdout with `--stdout`.

**What Stays Unchanged**

- Existing commands, aliases, and global flags remain backward compatible.
- `compose` CLI behavior remains unchanged; `gather` may expose/reuse compose internals for rendering.
- Generated temp cleanup is not part of MVP.

### Scope Summary

| File | Change |
|------|--------|
| `src/cli.rs` | Register `gather` built-in command and route execution. |
| `src/builtins/mod.rs` | Add `gather` module export. |
| `src/builtins/gather/` | New vertical module for CLI args, interactive mode, template generation, and source resolution. |
| `tests/gather.rs` | Add integration tests for CLI behavior, interactive mode, template generation, and output. |
| `README.md` | Document `gather` command, syntax, and examples. |
| `CHANGELOG.md` | Record the new command under unreleased/current version section. |

### Design Reference

→ See [design.md](./design.md)