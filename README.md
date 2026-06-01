# Agent Squire

`agent-squire` is a local CLI toolbox for humans and agents.

Binaries:

```bash
squire
agent-squire
asq
```

## What it provides

Agent Squire packages small, predictable local tools behind one CLI:

- inspect project files before reading them;
- extract Markdown structure and references;
- read exact line ranges;
- apply SEARCH/REPLACE patch blocks safely;
- run small agent-oriented utilities with structured output.

## Built-in commands

| Command | Alias | Purpose |
|---|---|---|
| `file-tree` | `view-tree` | Show a project directory tree for orientation. |
| `file-info` | `fileinfo` | Inspect file metadata and text/binary format. |
| `md-toc` | `mdtoc` | Show Markdown headings with 1-based line numbers. |
| `md-links` | `mdlinks` | Extract Markdown references and resolve file targets. |
| `read-lines` | `lines` | Read known 1-based line slices from one text file. |
| `patch-edit` | `patch` | Apply SEARCH/REPLACE patch blocks. |
| `imgweb` | — | Start a local web UI for composing multi-image prompts. |
| `now` | — | Print the current local date and time. |
| `list` | — | List built-in commands. |

Discover commands:

```bash
squire list
squire <command> --help
```

## Global options

```bash
squire --cwd /path/to/project file-tree .
squire --print json file-info README.md
squire file-info README.md --print json
```

`--print` is global and may appear before or after subcommands.

Supported modes:

- `compact`: default human-readable output;
- `json`: machine-readable JSON envelope;
- `ndjson`: reserved for streaming commands;
- `text`: plain body text where applicable;
- `raw`: passthrough-style output where applicable.

`--json` is a shortcut for `--print json`.

## Common workflows

### Project orientation

```bash
squire file-tree . --depth 3
squire file-info README.md src/cli.rs
squire md-toc README.md docs
```

Use this before selecting exact files or line ranges to read.

### Read exact line ranges

```bash
squire read-lines src/cli.rs --slice 1-80
squire lines README.md -s start-20 -s 80-120 -s end
```

Slice syntax:

| Syntax | Meaning |
|---|---|
| `N` | one line |
| `A-B` | inclusive range |
| `N~K` | line `N` with `K` lines of context on each side |
| `start` | first line |
| `end` | last line |

### Extract Markdown links

```bash
squire md-links README.md docs --workspace .
squire --print json md-links .
```

`md-links` extracts occurrence-level references for graph building.

Supported references:

- Markdown links/images: `[text](target)`, `![alt](target)`;
- Wiki links: `[[target]]`;
- inline code path refs: `` `src/main.rs` ``;
- angle refs: `<https://example.com>`, `<src/main.rs>`;
- SiYuan refs: `siyuan://...`, `((20260531010806-35bkoxa 'Title'))`.

File targets are resolved against the source file and workspace. Existing files are marked in the output.

Compact output groups by source file:

```text
# files=1 links=2 file=1 exists=1
@ README.md
L12|markdown|file|ok|"docs/intro.md#install"|"docs/intro.md"
L18|markdown|url|-|"https://example.com"
```

JSON output uses the standard envelope and is intended for graph consumers.

### Apply patches safely

`patch-edit` applies LRR-style SEARCH/REPLACE patch blocks.

Dry-run first:

```bash
squire patch-edit @file:fix.patch --dry-run --print json
```

Apply with explicit confirmation:

```bash
squire patch-edit @file:fix.patch --yes
```

Interactive mode opens `$EDITOR` / `$VISUAL` when configured, dry-runs first, optionally shows a unified diff, then asks before applying:

```bash
asq patch -i
```

Without `$EDITOR`, paste into the terminal and submit with a single `.` line.

Patch block capabilities:

- `SEARCH` / `REPLACE` exact matching;
- `CREATE` and `OVERWRITE`;
- optional 1-based line ranges;
- already-applied detection;
- ambiguous match detection;
- same-file multi-patch overlap detection;
- atomic writes with newline-style preservation;
- UTF-8 / UTF-8 BOM / GBK / Windows-1252 decoding fallback.

### Compose multi-image prompts

```bash
squire imgweb
```

`imgweb` starts a local browser UI for arranging uploaded images and generating a structured prompt.

## Input sources

Commands that accept text input support:

```text
@stdin        read from stdin
@file:path    read from a file
@env:NAME     read from an environment variable
@@file:path   pass literal "@file:path"
```

Examples:

```bash
cat fix.patch | squire patch-edit @stdin --dry-run
squire patch-edit @file:fix.patch --dry-run --print json
```

## Output contract

JSON mode uses a stable envelope shape:

```json
{
  "ok": true,
  "command": "md-links",
  "data": {},
  "warnings": [],
  "meta": {}
}
```

Prefer JSON mode when another tool or agent consumes the output.

## Design constraints

- Flat built-in commands first; avoid premature namespace splits.
- No thin wrappers for trivial shell commands.
- Built-ins live under `src/builtins/` as vertical command modules.
- CLI parsing is handled by `clap`.
- Agent-facing help and structured output are first-class.
- Existing command names and aliases should remain backwards compatible.

## Development

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt
```

Release checklist:

```bash
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt
```
