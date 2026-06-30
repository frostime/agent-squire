# Agent Squire

[![Crates.io](https://img.shields.io/crates/v/agent-squire)](https://crates.io/crates/agent-squire)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

A cross-platform CLI toolbox that packages common agentic-coding operations into structured, predictable commands.

Install it once, use it everywhere: `squire`, `agent-squire`, or `asq`.

## Why Agent Squire?

Agentic coding involves a small set of repetitive operations: orienting within a project, reading precise code ranges, inspecting structured data, assembling multi-source context, and applying changes safely. On different platforms, the same operation often requires different tools with inconsistent interfaces and output formats.

Agent Squire unifies these operations behind a single CLI with:

- **Cross-platform consistency** — same commands, same behavior on Windows, macOS, and Linux
- **Structured output** — every command supports `--print json` with a stable envelope
- **Safe mutations** — patch editing and file rearrangement with dry-run, overlap detection, and atomic writes

Humans benefit too: `gather` assembles project context for AI conversations, `img` handles clipboard images, and the uniform interface reduces context switching.

## Install

```bash
cargo install agent-squire
```

Binaries: `squire`, `agent-squire`, `asq`

## Quick Start

Orient before reading:

```bash
squire file-tree . --depth 3
squire md-toc README.md
squire file-info src/main.rs
```

Inspect structured data:

```bash
squire data-toc result.json
squire data-toc logs.jsonl --format jsonl
```

Read precise ranges:

```bash
squire read-range src/main.rs --range 1-80
```

Assemble context for an AI prompt:

```bash
squire gather file:src/main.rs tree:src cmd:"git status --short"
```

Apply patches safely:

```bash
squire patch-edit @file:fix.patch --dry-run
squire patch-edit @file:fix.patch --yes
```

Rearrange file contents with a state-transition DSL:

```bash
squire rearrange @file:plan.arr --dry-run
squire rearrange @file:plan.arr --yes
```

Save a clipboard image:

```bash
squire img
```

## Commands

| Command | Alias | Purpose |
|---|---|---|
| `file-tree` | `view-tree` | Directory tree for orientation |
| `file-info` | `fileinfo` | File metadata and format detection |
| `md-toc` | `mdtoc` | Markdown headings with line numbers |
| `data-toc` | `datatoc` | JSON/JSONL structure overview |
| `md-links` | `mdlinks` | Extract Markdown references |
| `md-backlinks` | `mdbacklinks` | Find backlinks to files |
| `read-range` | `range` | Read exact line ranges from text files |
| `patch-edit` | `patch` | Apply SEARCH/REPLACE patches safely |
| `rearrange` | `rearr` | Rewrite files with a state-transition DSL |
| `compose` | — | Render agent context templates |
| `gather` | — | Assemble files, trees, and command output |
| `img` | — | Save clipboard images or start web UI |
| `tmp` | `temp` | Create a temporary file or directory |
| `now` | — | Print current local date and time |
| `list` | — | List built-in commands |

### `file-tree` (`view-tree`)

`file-tree` gives you a compact project map before you open any file — useful for both humans and AI context windows.

```bash
squire file-tree . --depth 2
```

```text
./
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs
    ├── cli.rs
    └── builtins/
        ├── mod.rs
        ├── file_tree.rs
        └── patch_edit/
            ├── mod.rs
            └── parser.rs

Files: 40 | Directories: 34 | Total size: 286.4KB
```

Respects `.gitignore` by default; use `--no-gitignore` to see everything.

### `file-info` (`fileinfo`)

Quick metadata check before you feed a file into a text tool.

```bash
squire file-info README.md
```

```text
README.md | 9.2KB | utf-8 | lf | 389L
```

One line: filename, size, encoding, line endings, and line count. Catches binary files and encoding mismatches (GBK, Windows-1252) early.

### `md-toc` (`mdtoc`)

Get a navigable skeleton of any Markdown file.

```bash
squire md-toc README.md
```

```text
=== README.md ===
chars: 9340 | lines: 389

L1     # Agent Squire
L11      ## Why Agent Squire?
L23      ## Install
L31      ## Quick Start
L80      ## Commands
L350     ## Development
```

Jump straight to a heading with `squire read-range README.md --range 31`.

### `data-toc` (`datatoc`)

Peek inside large JSON/YAML/JSONL without loading the whole file.

```bash
squire data-toc result.json
```

```text
# data-toc result.json
format=json mode=structure-toc complete=true

$ object
├─ meta object
│  ├─ timestamp string
│  └─ version string
└─ users array<object> observed_items≈1
   └─ [] object
      ├─ id number
      ├─ name string
      └─ profile object
         ├─ email string
         └─ settings object
            ├─ notifications boolean
            └─ theme string

Suggested reads:
- jq '.users[0:5]' result.json
```

Shows type-tagged structure with array collapse and bounded scanning. Add `--examples` for sample values.

> **External dependency:**
> YAML support requires [`yq`](https://github.com/mikefarah/yq).

### `md-links` (`mdlinks`)

Extract and verify every reference in your Markdown.

```bash
squire md-links README.md
```

```text
# files=1 links=9 file_links=3 existing_file_links=0 missing_file_links=3
@ README.md links=9 file_links=3 missing_file_links=3
L3|url|image|url|"https://img.shields.io/crates/v/agent-squire"
L3|url|markdown|url|"https://img.shields.io/crates/v/agent-squire"
L259|missing|code_span|file|"src/main.rs"
```

Marks existing file targets as `ok` and missing ones as `missing`. Also extracts wiki links, inline code paths, and SiYuan refs.

### `read-range` (`range`)

Grab exactly the lines you need — no scrolling, no guesswork.

```bash
squire read-range src/cli.rs --range 1-10
```

```text
@@ Range Chunk │ src/cli.rs:1-10 (from-args=1-10) @@
  1 │ use std::ffi::OsString;
  2 │ use std::path::PathBuf;
  3 │ use std::process::ExitCode;
  4 │
  5 │ use anyhow::{Context, Result, bail};
  6 │ use clap::{Args, CommandFactory, Parser, Subcommand};
  7 │
  8 │ use crate::builtins;
  9 │ use crate::external;
 10 │ use crate::runtime::output::PrintMode;
```

Supports `N`, `A-B`, `N~K` (context on both sides), `start`, and `end`. Perfect when an AI says "look at line 50" and you want surrounding context.

### `patch-edit` (`patch`)

`patch-edit` is a cross-platform, validation-first file editing tool. It takes a SEARCH/REPLACE patch block, dry-runs it, checks for conflicts, and applies atomically — no shell required.

**Use it when:**
- You're in a restricted container where no `edit` tool is available
- You need a general-purpose editing primitive that works outside any specific harness
- You want dry-run validation, overlap detection, and already-applied detection before touching any file

Supports `@stdin`, `@file:path`, and literal input:

```bash
cat fix.patch | squire patch-edit @stdin --dry-run
squire patch-edit @file:fix.patch --yes
```

Patch block example:

```text
# src/utils.rs L10-L15
<<<<<<< SEARCH
fn old_helper(x: i32) -> i32 {
    x + 1
}
=======
fn new_helper(x: i32) -> i32 {
    x.saturating_add(1)
}
>>>>>>> REPLACE
```

```bash
squire patch-edit @file:fix.patch --dry-run
squire patch-edit @file:fix.patch --yes
```

Dry-run previews every change. Detects overlap, already-applied patches, and ambiguous matches. Also supports `CREATE` (new files) and `OVERWRITE` (full replacement).

Need the full patch DSL spec? `squire patch-edit --prompt` prints it.

### `rearrange` (`rearr`)

When an AI refactors a large file, it often rewrites the entire content — burning tokens and risking subtle drift. `rearrange` lets you describe the change as a precise state-transition: "take lines 1-60 (the API), lines 61-140 (the parser), and lines 141-end (the rest), then reorder them as API, rest, parser." The tool validates the pre-state, previews the outcome, and writes atomically.

**Why this matters:** instead of dumping a 200-line file into the context window and asking the model to "reorder these functions", you describe the operation in ~10 lines of DSL. Same result, far fewer tokens, zero hallucination risk.

Single-file reorder:

```text
arrange src/main.rs
  before api = 1-60, parser = 61-140, rest = 141-end
  after  api, rest, parser
end arrange
```

Cross-file extraction — move `parser` out of `src/main.rs` into a new `src/parser.rs`:

```text
arrange main = src/main.rs
  before api = 1-60, parser = 61-140, rest = 141-end
  after  api, rest
end arrange

arrange src/parser.rs
  before <missing>
  after  main::parser
end arrange
```

```bash
squire rearrange @file:plan.arr --dry-run
squire rearrange @file:plan.arr --yes
```

Validates the pre-state snapshot before touching anything. If the file changed since the snapshot was taken, the operation aborts — no half-finished edits.

Need the full DSL spec? `squire rearrange --prompt` prints it.

### `compose`

Fill a template with live project state — git status, file excerpts, command output — and get a bounded prompt file.

Template (`context.tpl.md`):

```markdown
## Request

${{stdin |> trim}}

## README

${{file: README.md |> lines: 1-40}}

## Git Status

${{exec: git status --short |> timeout: 5}}
```

```bash
squire compose -t context.tpl.md
```

Renders `${{...}}` interpolations into a temp file. Built-in truncation and spill files prevent accidental multi-megabyte output.

Need the full template syntax guide? `squire compose --prompt` prints it.

### `gather`

Assemble everything an AI needs to know about your project into one copy-pasteable prompt.

```bash
squire gather file:src/main.rs tree:src cmd:"git status --short"
```

Produces a single Markdown file with grouped, fenced blocks:

```markdown
--- FILE: src/main.rs ---
fn main() { ... }

--- TREE: src ---
src/
├── main.rs
└── lib.rs

--- CMD: git status --short ---
M src/main.rs
?? notes.md
```

One command, one file, ready to paste into any AI chat.

> **External dependency:** Interactive mode (`-i`) requires [`fzf`](https://github.com/junegunn/fzf).

Need the source syntax guide? `squire gather --prompt` prints it.

### `img`

Screenshot an error dialog, run `img`, and get a persistent PNG path you can reference in your next AI prompt.

```bash
squire img
# output: /tmp/agent-temp/asq-img-20260630T143052-abc123.png
```

Use `img --web` to start a browser UI for arranging multiple images and generating a structured prompt.

### `now`

Portable timestamp — no more `date` syntax archaeology across macOS and Linux.

```bash
squire now
# 2026-06-30 17:58:30 (+08:00)
```

Handy for timestamped filenames and log entries in scripts.

### `md-backlinks` (`mdbacklinks`)

Find which Markdown files link to a given file.

```bash
squire md-backlinks src/lib.rs
```

Scans the corpus for references to the target file. Useful when refactoring: rename a file and see every doc that needs updating.

### `tmp` (`temp`)

Create a scratch file or directory outside the current workspace.

```bash
squire tmp scratch.md
# /tmp/agent-temp/20260630T175830-scratch.md
```

Adds a timestamp prefix to avoid collisions. Useful when an agent or script needs transient storage.

### `list`

```bash
squire list
```

Prints every built-in command with its alias. Use `<command> --prompt` for the full DSL guide (patch-edit, rearrange, compose, gather).

Discover more:

```bash
squire <command> --help
squire <command> --prompt
```

## Global Options

```bash
squire --cwd /path/to/project file-tree .
squire --print json file-info README.md
squire file-info README.md --json
```

`--print` supports `compact` (default), `json`, `ndjson`, `text`, and `raw`. It may appear before or after subcommands.

Commands that accept text input support `@stdin`, `@file:path`, and `@env:NAME`.

## Output Contract

JSON mode uses a stable envelope:

```json
{
  "ok": true,
  "command": "md-links",
  "data": {},
  "warnings": [],
  "meta": {}
}
```

## Design

- Flat built-in commands first; no premature namespaces
- No thin wrappers for trivial shell commands
- Agent-facing help and structured output are first-class
- Existing command names and aliases remain backwards compatible

## Development

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt
```
