# Agent Squire

`agent-squire` is a local CLI toolbox for humans and agents.

Install:

```bash
cargo install agent-squire
```

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
| `read-range` | `range` | Read known 1-based line ranges from one text file. |
| `patch-edit` | `patch` | Apply SEARCH/REPLACE patch blocks. |
| `compose` | — | Render agent context templates into bounded UTF-8 output. |
| `gather` | — | Assemble files, trees, globs, and command output into one prompt. |
| `img` | — | Save clipboard images or start the image web UI. |
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
squire read-range src/cli.rs --range 1-80
squire range README.md -r start-20 -r 80-120 -r end
```

Range syntax:

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
# files=1 links=2 file_links=1 existing_file_links=1 missing_file_links=0
@ README.md links=2 file_links=1 missing_file_links=0
L12|ok|markdown|file|"docs/intro.md#install"=>"docs/intro.md"
L18|url|markdown|url|"https://example.com"
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

### Gather prompt context

`gather` assembles files, directory/glob file groups, directory trees, and command output into a fenced prompt body.

```bash
asq gather file:src/main.rs cmd:"git status --short"
asq gather --stdout file:src/main.rs:1-80
asq gather dir:src glob:"tests/*.rs" tree:src
asq gather -i
asq gather --no-gitignore dir:target
```

Default output is a persistent UTF-8 file under the system temp `agent-temp` directory:

```text
output: C:\Users\...\Temp\agent-temp\asq-gather-20260613T190000-....md
```

Source forms:

| Source | Meaning |
|---|---|
| `file:path` | Include one file. |
| `file:path:start-end` | Include an inclusive 1-based line range. |
| `dir:path` | Recursively expand files into one `DIR` group with nested `DIR-FILE` blocks. |
| `glob:pattern` | Expand matching files into one `GLOB` group with nested `GLOB-FILE` blocks. |
| `tree:path` | Include a compact directory structure. |
| `cmd:command` | Capture command stdout. |

Use `--no-gitignore` when directory expansion or interactive selectors should include files normally hidden by `.gitignore`.

Grouped output uses parent-qualified inner fences (`DIR-FILE-START`, `GLOB-FILE-START`) so expanded files are visually distinct from top-level `FILE` sources.

Interactive mode keeps path selection terminal-native:

```text
gather> file:   # opens fzf file selection, then edit> file:path allows adding :start-end
gather> dir:    # opens fzf directory selection, then edit> dir:path
gather> tree:   # opens fzf directory selection, then edit> tree:path
gather> /all    # toggle gitignored fzf candidates
gather> /list   # show selected sources
gather> /done   # render
gather> /exit   # quit without rendering

# after fzf selection:
edit> file:src/main.rs        # press Enter to accept
edit> file:src/main.rs:10-20  # or edit before accepting
```

### Work with images

```bash
squire img
squire img --web
```

`img` saves the current clipboard image as a persistent PNG and prints the local path. Use `img --web` to start the browser UI for arranging uploaded images and generating a structured prompt.

### Compose agent context

`compose` renders templates that pull content from stdin, files, environment variables, or guarded shell commands.

```bash
asq compose -t context.tpl.md
asq compose --template '${{file: README.md |> head: 80}}'
asq compose -t context.tpl.md --stdout
asq compose -t context.tpl.md --allow-exec
asq compose --prompt
```

By default, rendered content is written to a persistent UTF-8 file under the system temp `agent-temp` directory, and stdout reports the path:

```text
output: C:\Users\...\Temp\agent-temp\asq-compose-20260610T012244-....md
```

Use `--stdout` when the rendered body should be piped. JSON status never embeds the rendered body.

Large `exec:` streams are drained while the command runs. The rendered body keeps at most `--max-command-bytes` per stream; excess output is saved under the temp `agent-temp` directory as a spill artifact and referenced from the truncation marker / JSON `artifacts`. `--max-spill-bytes` defaults to `134217728` bytes as a per-run spill budget. Size truncation does not kill `exec:`; timeout does.

Template examples:

```md
## Request

${{stdin |> trim}}

## README

${{
  file: README.md
  |> lines: 1-END
  |> indent: 2
}}

## Git

${{exec: git status --short |> timeout: 5 |> stdout |> max-lines: 100 |> on-error: "git unavailable"}}
```

Command roles are normalized: source first, runtime controls and stream selectors before text transforms, and failure policies as recovery rules. Text transforms run left-to-right. No-argument commands may omit `:`; body-taking commands use `name: body`.

`--total-timeout` is the total render-phase wall-clock budget across all `${{...}}` interpolations. For `exec:`, the effective command timeout is the smaller of the local `timeout:` / global `--timeout` and the remaining total render budget.

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
