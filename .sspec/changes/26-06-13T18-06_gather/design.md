---
change: "gather"
created: 2026-06-13T18:06:22
---

# Design: gather

## Interface Contract

```text
asq gather [sources...] [options]
asq gather --interactive [options]

Sources (positional arguments):
  <path>                      Auto-detect type (see Auto-Detection rules)
  file:<path>                 File content
  file:<path>:<start>-<end>   File content with line range
  dir:<path>                  Directory content (list files)
  tree:<path>                 Directory tree (visual structure)
  glob:<pattern>              Glob pattern (expand and read matching files)
  cmd:<command>               Command output (execute and capture stdout)

Named flags (alternative to positional):
  -f, --file <path>           Add file (same as file:<path>)
  -d, --dir <path>            Add directory (same as dir:<path>)
  -t, --tree <path>           Add directory tree (same as tree:<path>)
  -g, --glob <pattern>        Add glob pattern (same as glob:<pattern>)
  -c, --cmd <command>         Add command output (same as cmd:<command>)

Interactive:
  -i, --interactive           Enter interactive mode; selector lines open fzf

Output:
      --stdout                Write rendered body to stdout
  -o, --output <path>         Write rendered body to explicit file path
      --overwrite             Allow replacing an existing --output file

Execution:
      --shell <mode>          auto | sh | bash | pwsh | powershell | cmd
      --timeout <seconds>     Default command timeout; default 30

Limits:
      --max-file-bytes <n>    Default 1048576
      --max-command-bytes <n> Default 1048576

Inspection:
      --prompt                Print an agent-facing long usage guide and exit
```

Mutual exclusion:

| Option set | Behavior |
|------------|----------|
| no `--stdout`, no `--output` | Render to `%TEMP%/agent-temp/asq-gather-<timestamp>-<uuid>.md`; stdout reports path/status. |
| `--stdout` | Rendered body goes to stdout; diagnostics/errors go to stderr. |
| `--output <path>` | Rendered body goes to path; fails if path exists unless `--overwrite`. |
| `--stdout` + `--output` | CLI usage error. |

## Input Parsing Rules

### Prefix Syntax

**Format**: `<prefix>:<content>`

**Parsing algorithm**:
1. Find the first `:` in the input string
2. Everything before `:` is the prefix (case-insensitive, trimmed)
3. Everything after `:` is the content (trim leading whitespace only)
4. If prefix is not recognized, treat entire input as auto-detect

**Prefix recognition** (case-insensitive):
- `file` → Source::File
- `dir` → Source::Dir
- `tree` → Source::Tree
- `glob` → Source::Glob
- `cmd` → Source::Command

### Content Parsing Rules

**Rule 1: Leading whitespace after colon is trimmed**
```
"file: src/main.rs"     → prefix="file", content="src/main.rs"
"file:  src/main.rs"    → prefix="file", content="src/main.rs"
"cmd: git status"       → prefix="cmd", content="git status"
```

**Rule 2: Content can contain colons**
```
"file:path:with:colons"     → prefix="file", content="path:with:colons"
"file:C:\Users\test.rs"     → prefix="file", content="C:\Users\test.rs"
"cmd:echo hello:world"      → prefix="cmd", content="echo hello:world"
```

**Rule 3: File path with line range**
```
"file:src/main.rs:10-20"    → prefix="file", content="src/main.rs:10-20"
```
The gather input content is then parsed as:
- Find the last `:` followed by `<number>-<number>` pattern
- Everything before is the path: `src/main.rs`
- The range is: start=10, end=20

When generating compose templates, line ranges MUST use compose's `lines` transform:
```
${{file: "src/main.rs" |> lines: 10-20}}
```
`gather` input syntax does not need to match compose syntax.

**Rule 4: Line range format**
```
<path>:<start>-<end>
```
- `start` and `end` are 1-based inclusive line numbers
- `start` must be ≥ 1
- `end` must be ≥ `start`
- Both must be valid integers

**Examples**:
```
"file:main.rs:1-10"     → path="main.rs", range=(1, 10)
"file:main.rs:5-5"      → path="main.rs", range=(5, 5)  // single line
"file:main.rs:abc-10"   → ERROR: invalid range
"file:main.rs:10-5"     → ERROR: end < start
```

### Auto-Detection Rules

When input has no recognized prefix, apply these rules in order:

1. **Ends with `/`**: Treat as `dir:<path>`
   ```
   "src/"          → dir:src/
   "src/utils/"    → dir:src/utils/
   ```

2. **Contains glob characters** (`*`, `?`, `[`, `]`): Treat as `glob:<pattern>`
   ```
   "src/**/*.rs"   → glob:src/**/*.rs
   "*.toml"        → glob:*.toml
   ```

3. **Path exists as file**: Treat as `file:<path>`
   ```
   "src/main.rs"   → file:src/main.rs (if file exists)
   ```

4. **Path exists as directory**: Treat as `dir:<path>`
   ```
   "src"           → dir:src (if directory exists)
   ```

5. **Otherwise**: Error "unknown source type, use prefix syntax"

## Interactive Mode

### Trigger

```bash
asq gather --interactive
asq gather -i
```

### Flow

```text
1. Print prompt: "gather> "
2. Read line from stdin
3. If EOF (Ctrl+D): exit loop, proceed to template generation
4. Parse input using Input Parsing Rules
5. If line contains only a selector prefix and colon (`file:`, `dir:`, `tree:`, `glob:`):
   a. Open fzf for that source type
   b. On selection, add selected source(s)
   c. On cancel, return to prompt without adding
6. If line contains only `cmd:`:
   a. Print continuation prompt: `cmd body> `
   b. Read exactly one command line
   c. Combine as `cmd:<body>` and parse using Input Parsing Rules
7. Validate source (check file exists, dir exists, etc.)
8. If valid: add to source list, print confirmation
9. If invalid: print error, continue loop
10. Go to step 1
```

### Interactive Entry Forms

```text
gather> file:src/main.rs          # add explicit file
gather> file:                     # open fzf file picker
gather> dir:                      # open fzf directory picker
gather> tree:                     # open fzf directory picker, add as tree
gather> glob:                     # open fzf file picker, add selected files as a glob group or explicit file group
gather> cmd:git status --short    # add explicit command
gather> cmd:                     # read one command body line
cmd body> cargo test --quiet
```

Interactive mode keeps fzf selection but avoids raw terminal Tab handling. Entering a selector line (`file:`, `dir:`, `tree:`, `glob:`) launches fzf.

### FZF Integration

**Trigger**: In interactive mode, Enter on selector-only lines:

| Input line | FZF selection pool | Result |
|---|---|---|
| `file:` | files under `--cwd` | add selected files as `file:<path>` sources |
| `dir:` | directories under `--cwd` | add selected directories as `dir:<path>` sources |
| `tree:` | directories under `--cwd` | add selected directories as `tree:<path>` sources |
| `glob:` | files under `--cwd` | add selected files as one grouped `GLOB`-style source using selected file paths as matched items |

**Selection**:
- Multi-select is allowed when supported by fzf (`--multi`).
- File preview uses a bounded text preview (`head -50` or internal equivalent).
- Directory preview lists immediate children.
- FZF runs with working directory set to `--cwd`.
- If fzf is not installed and a selector line is used, print: `fzf is required for interactive selection. Enter explicit prefix:path sources instead.`

### Interactive Session Example

```text
$ asq gather -i
gather> file:src/main.rs
  ✓ Added: file:src/main.rs
gather> file:src/lib.rs:10-20
  ✓ Added: file:src/lib.rs:10-20
gather> tree:src/
  ✓ Added: tree:src/
gather> cmd:git status
  ✓ Added: cmd:git status
gather> [Ctrl+D]
Generating template...
Output: /tmp/agent-temp/asq-gather-20260613T180622-abcd.md
```

## Directory And Glob Expansion

`dir:` and `glob:` sources render as grouped containers that show the matched items and the content for each matched file.

### `dir:<path>`

- Resolve `<path>` against `--cwd`.
- Discover files under the directory recursively.
- Respect normal project ignore behavior where practical (`.gitignore` and built-in skip rules), matching existing tree/file discovery conventions in this project.
- Sort matched file paths deterministically.
- Render one `DIR` fence containing a manifest plus one nested `FILE` block per matched text file.
- If no files match, render the container with `Matched files: (none)` and no nested `FILE` blocks.

```text
====== DIR-START: src/ ======
Matched files:
- src/main.rs
- src/lib.rs

====== FILE-START: src/main.rs ======
<content>
====== FILE-END ======

====== FILE-START: src/lib.rs ======
<content>
====== FILE-END ======
====== DIR-END ======
```

### `glob:<pattern>`

- Resolve `<pattern>` against `--cwd`.
- Expand to matching files.
- Sort matched file paths deterministically.
- Render one `GLOB` fence containing a manifest plus one nested `FILE` block per matched text file.
- If no files match, render the container with `Matched files: (none)` and no nested `FILE` blocks.

```text
====== GLOB-START: src/**/*.rs ======
Matched files:
- src/main.rs
- src/lib.rs

====== FILE-START: src/main.rs ======
<content>
====== FILE-END ======

====== FILE-START: src/lib.rs ======
<content>
====== FILE-END ======
====== GLOB-END ======
```

Binary files are refused using compose file-source behavior. A refused file causes the gather run to fail unless a future design adds per-file skip policy.

## Output Format

### Delimiter Specification

**Pattern**: `====== <TYPE>-START: <label> ======` ... `====== <TYPE>-END ======`

**Rules**:
1. Delimiter line starts with `====== ` (6 equals + space)
2. Followed by type keyword: `FILE`, `DIR`, `TREE`, `GLOB`, `CMD`
3. Followed by `-START: ` or `-END`
4. For START: followed by label (path, pattern, or command)
5. For END: no label, just `======`
6. Each delimiter line ends with ` ======` (space + 6 equals)

**Type keywords**:
| Source Type | START | END |
|-------------|-------|-----|
| File | `====== FILE-START: <path> ======` | `====== FILE-END ======` |
| File with range | `====== FILE-START: <path>:<start>-<end> ======` | `====== FILE-END ======` |
| Directory | `====== DIR-START: <path> ======` | `====== DIR-END ======` |
| Directory tree | `====== TREE-START: <path> ======` | `====== TREE-END ======` |
| Glob | `====== GLOB-START: <pattern> ======` | `====== GLOB-END ======` |
| Command | `====== CMD-START: <command> ======` | `====== CMD-END ======` |

**Examples**:
```
====== FILE-START: src/main.rs ======
fn main() {
    println!("Hello");
}
====== FILE-END ======

====== FILE-START: src/main.rs:10-20 ======
fn main() {
    println!("Hello");
}
====== FILE-END ======

====== TREE-START: src/ ======
src/
├── main.rs
└── lib.rs
====== TREE-END ======

====== CMD-START: git status ======
On branch main
====== CMD-END ======
```

### Template Generation

For each source, generate a compose template segment:

```rust
fn generate_segment(source: &Source) -> String {
    match source {
        Source::File { path, range } => {
            let label = match range {
                Some((start, end)) => format!("{}:{}-{}", path.display(), start, end),
                None => path.display().to_string(),
            };
            let file_ref = match range {
                Some((start, end)) => format!(
                    "${{{{file: {} |> lines: {}-{}}}}}",
                    quote_compose_body(&path.display().to_string()),
                    start,
                    end
                ),
                None => format!(
                    "${{{{file: {}}}}}",
                    quote_compose_body(&path.display().to_string())
                ),
            };
            format!(
                "====== FILE-START: {} ======\n{}\n====== FILE-END ======",
                label, file_ref
            )
        }
        Source::Dir { path } => {
            // Expand directory to sorted file list first, then generate a DIR fence
            // with a manifest and nested FILE blocks. Each nested file uses a compose
            // file source with JSON-quoted body.
            generate_dir_segment(path)
        }
        Source::Tree { path } => {
            let command = format!(
                "tree {} || find {} -print | head -50",
                shell_quote_path(path),
                shell_quote_path(path)
            );
            format!(
                "====== TREE-START: {} ======\n${{{{exec: {}}}}}\n====== TREE-END ======",
                path.display(), quote_compose_body(&command)
            )
        }
        Source::Glob { pattern } => {
            // Expand glob to sorted file list first, then generate a GLOB fence
            // with a manifest and nested FILE blocks. Each nested file uses a compose
            // file source with JSON-quoted body.
            generate_glob_segment(pattern)
        }
        Source::Command { command } => {
            format!(
                "====== CMD-START: {} ======\n${{{{exec: {}}}}}\n====== CMD-END ======",
                command, quote_compose_body(command)
            )
        }
    }
}
```

## Source Resolution

| Source | Behavior |
|--------|----------|
| `file:<path>` | Resolve relative path against `cwd`, read one file, decode text (UTF-8 BOM, UTF-8, GBK, Windows-1252 fallback), reject binary/null bytes. |
| `file:<path>:<start>-<end>` | Same as above, but only read lines `<start>` to `<end>` (inclusive, 1-based). |
| `dir:<path>` | Resolve relative path against `cwd`, recursively expand to sorted matching files, render grouped manifest plus nested file content blocks. |
| `tree:<path>` | Resolve relative path against `cwd`, generate directory tree using an internal walker or command-backed fallback. |
| `glob:<pattern>` | Resolve relative pattern against `cwd`, expand to sorted matching files, render grouped manifest plus nested file content blocks. |
| `cmd:<command>` | Run through selected shell in `cwd`, capture stdout. `gather` enables compose exec internally for explicit command-backed sources. |

## CLI Argument Quoting

When using CLI flags, arguments with spaces must be quoted:

```bash
# Correct
asq gather -c "git status"
asq gather --cmd "echo hello world"

# Incorrect (will be split into multiple arguments)
asq gather -c git status
```

When using positional arguments, the same quoting rules apply:

```bash
# Correct
asq gather "cmd:git status"
asq gather cmd:"git status"

# Both work, but prefer the first for clarity
```

## Source Ordering

Positional sources are processed in the order they appear:

```bash
# File first, then command
asq gather file:src/main.rs cmd:git status

# Command first, then file
asq gather cmd:git status file:src/main.rs
```

Named flags may be collected by source type; cross-flag ordering is not part of the contract.

## Complete Example

**Input**:
```bash
asq gather file:src/main.rs:10-20 tree:src/ cmd:"git status"
```

**Generated template**:
```
====== FILE-START: src/main.rs:10-20 ======
${{file: "src/main.rs" |> lines: 10-20}}
====== FILE-END ======

====== TREE-START: src/ ======
${{exec: "tree src/ || find src/ -print | head -50"}}
====== TREE-END ======

====== CMD-START: git status ======
${{exec: "git status"}}
====== CMD-END ======
```

**Output** (rendered by compose):
```
====== FILE-START: src/main.rs:10-20 ======
fn main() {
    println!("Hello, world!");
    // ... more code
}
====== FILE-END ======

====== TREE-START: src/ ======
src/
├── main.rs
├── lib.rs
└── utils/
    └── helper.rs
====== TREE-END ======

====== CMD-START: git status ======
On branch main
Changes not staged for commit:
  (use "git add <file>..." to update what will be committed)
        modified:   src/main.rs
====== CMD-END ======
```

## Error Handling

| Error | Message | Recovery |
|-------|---------|----------|
| Invalid prefix | "Unknown prefix: <prefix>. Use file/dir/tree/glob/cmd" | Continue interactive loop |
| File not found | "File not found: <path>" | Continue interactive loop |
| Directory not found | "Directory not found: <path>" | Continue interactive loop |
| Invalid range | "Invalid range: <range>. Use format: start-end" | Continue interactive loop |
| Binary file | "Binary file refused: <path>" | Continue interactive loop |
| Command failed | "Command failed: <command>" | Continue interactive loop |
| fzf not installed | "fzf is required for interactive selection" | Continue interactive loop |
| No sources | "No sources specified" | Exit with error |

## Module Blueprint

```text
src/builtins/gather/
  mod.rs          CLI args + run orchestration
  model.rs        Source enum, Segment struct, error types
  parser.rs       Input parsing (prefix syntax, auto-detection, range parsing)
  interactive.rs  Interactive prompt + fzf selector integration
  template.rs     Template generation (delimiter format)
  sources.rs      Source resolution (file/dir/tree/glob/cmd readers)
  output.rs       Compose execution + status reporting
```

## Integration with Compose

`gather` generates a compose template and executes `compose` with it:

```rust
fn run_gather(sources: Vec<Source>, options: GatherOptions) -> Result<()> {
    // Generate compose template
    let template = generate_template(&sources);
    
    // Write template to temp file
    let temp_template = write_temp_template(&template)?;
    
    // Execute compose internals with generated template.
    // Explicit gather command sources may require exec, so allow_exec is true
    // when the generated template contains command-backed compose sources.
    let compose_args = ComposeArgs {
        template_file: temp_template.path(),
        stdout: options.stdout,
        output: options.output,
        overwrite: options.overwrite,
        allow_exec: template_requires_exec(&template),
        shell: options.shell,
        timeout: options.timeout,
        max_file_bytes: options.max_file_bytes,
        max_command_bytes: options.max_command_bytes,
        ..Default::default()
    };
    
    run_compose(compose_args)
}
```

## Template Body Quoting

Generated compose source bodies MUST be JSON string quoted instead of inserted raw.

Reason: compose raw bodies are parsed until template control tokens such as `|>` or `}}`. A valid path or command can contain those byte sequences, quotes, backslashes, or newlines. JSON quoting preserves the user's body as data rather than accidentally turning it into compose syntax.

```text
Input path: weird}}name.rs
Unsafe template: ${{file: weird}}name.rs}}
Safe template:   ${{file: "weird}}name.rs"}}

Input command: printf 'a |> b'
Unsafe template: ${{exec: printf 'a |> b'}}
Safe template:   ${{exec: "printf 'a |> b'"}}
```

Implementation contract:

```rust
fn quote_compose_body(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization cannot fail")
}
```

## Agent Prompt

`--prompt` prints a concise built-in guide for agents, similar to `asq compose --prompt`.

Guide outline:

```text
# Squire gather guide

## Command
- default temp output
- --stdout pipeline mode
- --interactive mode with fzf selector lines

## Sources
- file:path
- file:path:start-end
- dir:path
- tree:path
- glob:pattern
- cmd:command

## Input syntax
- prefix:content format
- Leading whitespace after colon is trimmed
- Content can contain colons
- Auto-detection when no prefix

## Interactive mode
- prefix:content syntax
- selector lines open fzf: file:, dir:, tree:, glob:
- cmd: can read one body line
- Ctrl+D to finish

## Output format
- ====== FILE-START: path ======
- content
- ====== FILE-END ======

## Examples
- gather file:src/main.rs
- gather file:src/main.rs:10-20
- gather --interactive
- gather file:src/main.rs cmd:"git status"
```