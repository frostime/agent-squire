# `<this-cmd>` PRD / Design Spec

> Status: Draft v2  
> Target: local coding agents / CLI implementers  
> Command placeholder: `<this-cmd>`  
> Product type: deterministic context composer for agents

---

## 1. Summary

`<this-cmd>` renders agent-facing context templates.

A template contains interpolation blocks using `${{ ... }}`:

```txt
${{stdin:}}

${{file: src/app.ts |> lines: 40-END |> indent: 2}}

${{exec: git status --short |> timeout: 5 |> stdout: |> max-lines: 100}}
```

The renderer resolves each source, applies modifiers left-to-right, enforces limits, handles failures according to policy modifiers, and writes rendered output to stdout or a file.

This is not a general template engine. It is a deterministic context composer for local agents.

Core promise:

```txt
stdin / files / command outputs / environment
        ↓
bounded + reproducible + auditable rendering
        ↓
context document for LLM agents, patch engines, review artifacts, or debugging
```

---

## 2. Key Design Decisions

1. Use `${{ ... }}` instead of `{{ ... }}` to avoid collision with common template engines.
2. Source and modifier syntax is unified:

```txt
<cmd-name>:<space>*(<body>|<args>)
```

Examples:

```txt
stdin:
file: README.md
exec: git status --short
lines: 1-END
head: 100
indent: 2
timeout: 30
fallback: "not found"
```

3. Modifier separator is `|>` instead of `|`.
4. No `glob:` in this version.
5. No source in this version may expand into a sequence.
6. Use `--cwd`, not `--root`.
7. Default timeout is 30 seconds.
8. Support `--print compact|json` for diagnostics/status formatting.
9. `exec:` is disabled unless `--allow-exec` is passed.

---

## 3. Goals

### Product goals

1. Provide a stable cross-platform way to compose agent context.
2. Avoid fragile shell glue such as `cat`, `sed`, `awk`, `grep`, `head`, `tail`, PowerShell loops, heredocs, and command substitution.
3. Make every included source explicit.
4. Bound output size to prevent context explosion.
5. Support safe command execution with opt-in `--allow-exec`, timeout, and output limits.
6. Support text transforms: line slicing, char slicing, head/tail, trim, oneline, indentation, stdout/stderr selection, fallback behavior.
7. Produce deterministic output when all inputs are unchanged.
8. Be easy for coding agents to generate correctly.

### Non-goals

Do not implement in this version:

```txt
glob expansion
sources that expand into multiple source items
if / else
for loops
macros
variables
template inheritance
general expression evaluation
recursive templates
network fetching
remote source loading
semantic code parsing
secret management
```

---

## 4. Mental Model

Each interpolation block is:

```txt
${{ source |> modifier |> modifier |> failure-policy }}
```

Conceptually:

```txt
source -> transforms -> stream selection -> limits -> failure handling -> rendered text
```

Examples:

```txt
${{stdin:}}

${{file: README.md}}

${{file: src/app.ts |> lines: 40-80 |> indent: 2}}

${{exec: npm test -- --runInBand |> timeout: 120 |> stdout: |> max-lines: 400}}
```

Layer examples:

| Layer | Examples | Purpose |
|---|---|---|
| source | `stdin:`, `file: path`, `exec: command`, `env: NAME` | Where content comes from |
| selection | `lines: 40-80`, `slice: BEG-200`, `head: 100`, `tail-char: 4000` | Select part of content |
| transform | `trim:`, `oneline:`, `indent: 2` | Transform text |
| stream | `stdout:`, `stderr:` | Select command stream |
| limit | `max-lines: 100`, `max-bytes: 100000` | Bound output |
| failure policy | `fallback: "..."`, `on-error: "..."`, `on-404: "..."` | Decide what to render on failure |
| runtime policy | `timeout: 5` | Control dynamic sources |

---

## 5. CLI Interface

### Command shape

Preferred:

```bash
<this-cmd> render --template-file <path> [options]
<this-cmd> render --template <string> [options]
```

A single-command binary may also expose:

```bash
<this-cmd> --template-file <path> [options]
<this-cmd> --template <string> [options]
```

This spec uses the subcommand form.

### Template input

Exactly one of these is required:

```txt
-t, --template-file <path>
--template <string>
```

Rules:

1. `--template-file` reads a template from a file.
2. `--template` treats the argument as literal template text.
3. They are mutually exclusive.
4. Do not infer whether a string is a path.

Do not support `--template-file -` in MVP. Reading template from stdin conflicts with `${{stdin:}}` unless a second input channel is introduced.

### Working directory

```txt
--cwd <path>
```

Rules:

1. Relative file paths resolve against `--cwd`.
2. `exec:` commands run with current working directory set to `--cwd`.
3. If omitted, use process current working directory.
4. `--cwd` is not a sandbox. It is a resolution base and execution directory.
5. Absolute paths are allowed unless a future security mode disables them.

Use `--cwd`, not `--root`, because `--root` implies sandboxing and path confinement.

### Output

```txt
-o, --output <path>
--overwrite
```

Rules:

1. If `--output` is omitted, write rendered content to stdout.
2. If `--output` is provided, write rendered content to that file.
3. If output file exists, fail unless `--overwrite` is provided.
4. Writes to `--output` must be atomic where possible.
5. Failed rendering must not leave a partial output file.
6. Diagnostics must go to stderr, not stdout.

### Diagnostic print mode

```txt
--print compact|json
```

Default:

```txt
--print compact
```

`--print` controls diagnostics/status formatting, not the rendered document.

`compact` example:

```txt
error: file not found at template.md:12:1: src/missing.ts
```

`json` example:

```json
{
  "ok": false,
  "error": {
    "code": "source_404",
    "message": "File not found: src/missing.ts",
    "source": "file: src/missing.ts",
    "location": {
      "template": "template.md",
      "line": 12,
      "column": 1
    }
  }
}
```

Rules:

1. Rendered output is never JSON-wrapped by `--print json`.
2. `--print json` affects errors, warnings, and optional final status.
3. For full audit data, use `--manifest`.

### Execution control

```txt
--allow-exec
--shell auto|sh|bash|pwsh|powershell|cmd
--timeout <seconds>
--total-timeout <seconds>
```

Rules:

1. `exec:` is disabled unless `--allow-exec` is provided.
2. `--timeout` sets the default timeout for dynamic sources. Default: `30`.
3. Local `timeout: N` overrides CLI `--timeout`.
4. `--total-timeout` optionally limits the whole render process.
5. `--shell auto` is default.
6. `--shell` controls `exec:` execution.

Suggested `--shell auto` behavior:

| Platform | Shell |
|---|---|
| Windows | `pwsh` if available, else `powershell.exe`, else `cmd.exe` |
| Unix-like | `sh` |
| Explicit `bash` | `bash` |
| Explicit `pwsh` | `pwsh` |
| Explicit `powershell` | `powershell.exe` |
| Explicit `cmd` | `cmd.exe` |

`<this-cmd>` should not translate commands between Bash and PowerShell.

### Global limits

```txt
--max-lines <number>
--max-bytes <number>
--max-file-bytes <number>
--max-command-bytes <number>
--fail-on-truncated
```

Recommended defaults:

```txt
--timeout 30
--max-file-bytes 1048576
--max-command-bytes 1048576
--max-lines unlimited
--max-bytes unlimited
```

Rules:

1. Source-specific modifiers override global limits.
2. If output is truncated, insert a truncation marker unless `--fail-on-truncated` is set.
3. If `--fail-on-truncated` is set, truncation becomes failure case `limit`.
4. Byte truncation must preserve valid UTF-8.

Suggested markers:

```txt
[<this-cmd>: truncated after 100 lines]
[<this-cmd>: truncated after 1048576 bytes]
```

### Manifest

```txt
--manifest <path>
```

Writes JSON audit data about the render.

Manifest should include cwd, template, output, options, sources, source locations, command exit codes, truncation info, warnings, and errors if applicable.

Example:

```json
{
  "ok": true,
  "cwd": "/repo",
  "template": { "kind": "file", "path": "context.tpl.md" },
  "output": { "kind": "file", "path": "context.md" },
  "options": {
    "timeout_seconds": 30,
    "shell": "sh",
    "allow_exec": true
  },
  "sources": [
    {
      "index": 0,
      "kind": "stdin",
      "raw": "${{stdin:}}",
      "location": { "line": 5, "column": 1 },
      "ok": true,
      "bytes": 120,
      "lines": 4,
      "truncated": false
    }
  ],
  "warnings": []
}
```

### Recommended additional options

```txt
--check
--list-sources
--no-color
--color auto|always|never
```

`--check` parses and validates the template without resolving sources.

`--list-sources` prints discovered sources without reading files or running commands.

With `--print compact`:

```txt
1  stdin:                          template.md:5:1
2  file: src/app.ts                template.md:11:1
3  exec: git status --short        template.md:19:1
```

With `--print json`:

```json
{
  "ok": true,
  "sources": [
    {
      "index": 0,
      "kind": "stdin",
      "argument": "",
      "location": { "line": 5, "column": 1 }
    }
  ]
}
```

---

## 6. Template Syntax

### Interpolation block

```txt
${{ <expression> }}
```

Expression:

```txt
command
command |> command
command |> command |> command
```

Each command:

```txt
<cmd-name>:<optional whitespace><body>
```

Examples:

```txt
${{stdin:}}

${{file: README.md}}

${{file: src/app.ts |> lines: 40-80 |> indent: 2}}

${{exec: git status --short |> timeout: 5 |> stdout: |> max-lines: 100}}
```

Whitespace rules:

1. Whitespace immediately inside `${{` and `}}` is ignored.
2. Whitespace around `|>` is ignored.
3. Whitespace after `:` is ignored by default.
4. Command body preserves internal whitespace after trimming outer whitespace.
5. Use quoted bodies for exact preservation or special characters.

### Pseudogrammar

```txt
Interpolation = "${{" WS Expression WS "}}"

Expression = Command (WS "|>" WS Command)*

Command = Name ":" WS Body?

Name = [A-Za-z_][A-Za-z0-9_-]*

Body =
    unquoted text until "|>" or "}}"
  | JSON string
  | empty
```

Valid command names:

```txt
stdin
file
exec
env

lines
slice
head
head-char
tail
tail-char
trim
oneline
indent
max-lines
max-bytes
stdout
stderr
fallback
on-error
on-404
timeout
```

Unknown command names fail unless a future worker/plugin mechanism is implemented.

### Quoted body

A command body may be a JSON string.

Examples:

```txt
${{file: "docs/My File.md"}}

${{fallback: "not available"}}

${{exec: "git diff -- src/app.ts | head -300" |> timeout: 10}}
```

Rules:

1. JSON string unescaping applies.
2. Quoted body can contain `|>`.
3. Quoted body can contain `}}`.
4. Invalid JSON string fails with `invalid_quoted_body`.

### Escaping literal interpolation

Recommended escaping:

```txt
\${{ renders as ${{
\}} renders as }}
```

Example:

```txt
Use \${{file: path\}} in docs.
```

Rendered:

```txt
Use ${{file: path}} in docs.
```

### Nested interpolation

Nested interpolation is not supported.

Invalid:

```txt
${{file: ${{env: SOME_PATH}}}}
```

Error:

```txt
nested_interpolation_not_supported
```

---

## 7. Built-in Sources

A source is the first command in an expression.

### `stdin:`

```txt
${{stdin:}}
```

Reads stdin.

Rules:

1. Body must be empty.
2. Repeated `stdin:` references use cached stdin content.
3. If stdin is empty, render empty string.
4. If stdin is unavailable in an interactive context, implementation may read empty content or fail with `stdin_unavailable`; behavior must be documented.

Examples:

```txt
${{stdin:}}

${{stdin: |> trim:}}

${{stdin: |> oneline:}}
```

### `file: <path>`

```txt
${{file: <path>}}
```

Reads one text file.

Rules:

1. Relative paths resolve against `--cwd`.
2. Absolute paths are allowed.
3. Missing file is failure case `404`.
4. Binary files fail with failure case `binary`.
5. Invalid UTF-8 fails with failure case `encoding`.
6. `file:` does not expand globs.
7. `file:` reads exactly one file.
8. File content is preserved unless modifiers transform it.
9. If `--manifest` is enabled, include path, size, selected range, and hash if feasible.

Examples:

```txt
${{file: README.md}}

${{file: src/app.ts |> lines: 40-END |> indent: 2}}

${{file: "docs/My File.md" |> head: 100}}
```

### `exec: <command>`

```txt
${{exec: <command>}}
```

Runs a command and captures stdout/stderr.

Rules:

1. Disabled unless `--allow-exec` is provided.
2. Uses shell from `--shell`.
3. Runs with cwd set to `--cwd`.
4. Default timeout is CLI `--timeout`, default 30 seconds.
5. Local `timeout:` modifier overrides timeout.
6. Non-zero exit code is failure case `error`.
7. Timeout is failure case `timeout`.
8. Default selected stream is stdout.
9. Use `stderr:` to select stderr.
10. `stdout:` and `stderr:` are mutually exclusive in MVP.
11. Command output is subject to command byte limits and local max modifiers.

Examples:

```txt
${{exec: git status --short |> timeout: 5}}

${{exec: npm test -- --runInBand |> timeout: 120 |> stdout: |> max-lines: 400}}

${{exec: "node script.js" |> stderr: |> max-lines: 100}}
```

Security:

`exec:` makes the template executable. Never run it without explicit `--allow-exec`.

### `env: <name>`

Recommended MVP or P1 source.

```txt
${{env: NODE_ENV}}
```

Rules:

1. Reads one environment variable.
2. Missing env var is failure case `404`.
3. Manifest should include env var name.
4. Manifest should not include env var value unless a future option allows it.

Examples:

```txt
${{env: NODE_ENV}}

${{env: CI |> fallback: "false"}}
```

### No `glob:` in this version

Do not implement:

```txt
${{glob: src/**/*.ts}}
```

Reason:

1. It expands one source into a sequence.
2. Sequence expansion introduces ordering, limits, formatting, partial failures, and output explosion risks.
3. This version keeps each interpolation block as one source producing one text value.

Agents may use explicit commands when needed:

```txt
${{exec: find src -name "*.ts" |> stdout: |> max-lines: 100}}
```

That requires `--allow-exec`, which is the correct trust boundary.

---

## 8. Built-in Modifiers

Modifiers are commands after `|>`. They use the same syntax as sources:

```txt
name: body
```

Modifiers are applied left-to-right, except failure-policy modifiers may be collected and applied on failure.

### Range notation

Used by `lines:` and `slice:`.

Endpoints support:

```txt
<number>
BEG
END
```

`BEG` means beginning. `END` means end.

### `lines: <start>-<end>`

Selects inclusive 1-based line range.

Examples:

```txt
lines: 1-20
lines: BEG-20
lines: 40-END
lines: BEG-END
```

Rules:

1. Line numbers are 1-based.
2. Range is inclusive.
3. `BEG` means line 1.
4. `END` means last line.
5. Out-of-range is failure case `range`.
6. `start > end` is failure case `range`.

Example:

```txt
${{file: src/app.ts |> lines: 40-80}}
```

### `slice: <start>-<end>`

Selects inclusive character range.

Examples:

```txt
slice: BEG-1000
slice: 100-500
slice: 200-END
```

Rules:

1. Character offsets are 1-based.
2. Implementation must document whether the unit is Unicode scalar value, grapheme cluster, or another safe character unit.
3. Must not corrupt UTF-8.
4. `BEG` means first character.
5. `END` means last character.
6. Out-of-range is failure case `range`.

Use `max-bytes:` for byte limits.

### `head: <number>`

Keeps first N lines.

```txt
${{file: README.md |> head: 80}}
```

### `tail: <number>`

Keeps last N lines.

```txt
${{file: app.log |> tail: 200}}
```

### `head-char: <number>`

Keeps first N characters.

```txt
${{file: large.txt |> head-char: 4000}}
```

### `tail-char: <number>`

Keeps last N characters.

```txt
${{file: app.log |> tail-char: 8000}}
```

Rules for head/tail modifiers:

1. N must be a non-negative integer.
2. Character operations must not corrupt UTF-8.
3. If N exceeds content size, keep all content.
4. These modifiers are successful truncating transforms; they are not failure unless future strict mode says otherwise.

### `trim:`

Trims leading and trailing whitespace from entire content.

```txt
${{exec: git rev-parse --abbrev-ref HEAD |> trim:}}
```

### `oneline:`

Converts content to one line.

Recommended behavior:

1. Trim leading/trailing whitespace.
2. Replace each run of whitespace, including newlines and tabs, with one space.

```txt
${{file: title.txt |> oneline:}}
```

### `indent: <number>`

Prefixes each line with N spaces.

```txt
${{file: src/app.ts |> lines: 40-80 |> indent: 2}}
```

Rules:

1. N must be >= 0.
2. Empty lines are also prefixed.
3. `indent: 0` is no-op.

### `max-lines: <number>`

Limits content to at most N lines.

```txt
${{exec: npm test |> max-lines: 300}}
```

Rules:

1. If content exceeds N lines, truncate and mark as truncated.
2. Insert truncation marker unless `--fail-on-truncated` is set.
3. Truncation is not failure unless `--fail-on-truncated` is set.

### `max-bytes: <number>`

Limits content to at most N UTF-8 bytes.

```txt
${{file: large.log |> max-bytes: 200000}}
```

Rules:

1. Must not cut invalid UTF-8.
2. Insert truncation marker unless `--fail-on-truncated` is set.
3. Truncation is not failure unless `--fail-on-truncated` is set.

### `stdout:`

Selects stdout as rendered content.

```txt
${{exec: npm test |> stdout:}}
```

### `stderr:`

Selects stderr as rendered content.

```txt
${{exec: npm test |> stderr:}}
```

Rules:

1. Default stream for `exec:` is stdout.
2. `stdout:` and `stderr:` are mutually exclusive in MVP.
3. Applying `stdout:` or `stderr:` to non-stream sources is failure case `modifier`.
4. If selected stream is empty, render empty string unless fallback applies.

### `timeout: <seconds>`

Sets timeout for dynamic source resolution.

```txt
${{exec: npm test |> timeout: 120}}
```

Rules:

1. Applies to `exec:` and future dynamic sources.
2. Overrides CLI `--timeout`.
3. Applying `timeout:` to `file:` or `stdin:` should be accepted as no-op in MVP to simplify template reuse.
4. Timeout must be positive.

Failure case:

```txt
timeout
```

---

## 9. Fallback and Failure Policy

Failure policy modifiers define what to render when source resolution or modifier application fails.

### Failure case vocabulary

| Case | Meaning |
|---|---|
| `404` | Resource not found, such as missing file or env var |
| `error` | Execution error, non-zero exit, permission error, invalid command |
| `timeout` | Dynamic source timed out |
| `range` | Invalid or out-of-range `lines:` / `slice:` |
| `encoding` | Invalid text encoding |
| `binary` | Binary file refused |
| `limit` | Limit exceeded with `--fail-on-truncated` |
| `modifier` | Modifier invalid for source |
| `parse` | Expression parse error; generally unrecoverable inside interpolation |

Parse errors should fail the whole template and should not be recoverable by fallback inside the same invalid expression.

### `fallback: <value>`

Fallback for any recoverable failure.

```txt
${{file: notes.md |> fallback: ""}}

${{exec: git describe --tags |> fallback: "unknown"}}
```

Rules:

1. Applies to any recoverable failure unless a more specific `on-<case>:` is provided.
2. Body is rendered as literal string.
3. JSON quoted body is recommended for empty string or special characters.

### `on-<case>: <value>`

Case-specific fallback.

Examples:

```txt
on-404: "not found"
on-error: "command failed"
on-timeout: "timed out"
on-range: "range invalid"
```

Rules:

1. `on-<case>:` applies only to that failure case.
2. More specific `on-<case>:` takes precedence over `fallback:`.
3. If no matching fallback exists, rendering fails.
4. `on-error:` covers execution non-zero exit and execution errors.
5. `on-404:` covers missing file and missing env var.

Examples:

```txt
${{file: optional.md |> on-404: ""}}

${{exec: git describe --tags |> timeout: 5 |> on-error: "unknown" |> on-timeout: "unknown"}}
```

### Ordering of fallback modifiers

Fallback modifiers may appear anywhere after the source, but they are policies.

Recommended implementation:

1. Parse all modifiers.
2. Separate policy modifiers from transform modifiers.
3. Resolve source and apply transforms.
4. If a failure occurs, select fallback by case.
5. If fallback selected, render fallback literal and do not apply remaining transforms after the failure.

Example:

```txt
${{file: missing.md |> lines: 1-10 |> on-404: "missing"}}
```

If source fails with `404`, render:

```txt
missing
```

Example:

```txt
${{file: short.md |> lines: 100-200 |> on-range: "out of range"}}
```

If file exists but range fails, render:

```txt
out of range
```

---

## 10. Error Handling

### Exit codes

| Code | Meaning |
|---:|---|
| 0 | Success |
| 1 | General render failure |
| 2 | Invalid CLI usage |
| 3 | Template parse error |
| 4 | Source resolution failure |
| 5 | Exec disabled |
| 6 | Timeout |
| 7 | Exec non-zero exit |
| 8 | Output write error |
| 9 | Limit failure |
| 10 | Internal error |

### Error object for `--print json`

```json
{
  "ok": false,
  "error": {
    "code": "source_404",
    "case": "404",
    "message": "File not found: src/missing.ts",
    "raw": "${{file: src/missing.ts}}",
    "location": {
      "template": "context.tpl.md",
      "line": 12,
      "column": 1
    }
  }
}
```

Required fields:

```txt
ok
error.code
error.message
```

Recommended fields:

```txt
error.case
error.raw
error.location
error.details
```

### Partial output

1. If writing to stdout, buffering is preferred over streaming.
2. If writing to `--output`, always render to memory or temp file first.
3. If any unrecovered failure occurs, do not write output file.
4. If a failure is recovered by fallback, rendering continues.

---

## 11. Security Model

### Exec trust boundary

Templates using `exec:` are executable templates.

Rules:

1. `exec:` is disabled by default.
2. Require `--allow-exec`.
3. Apply timeout.
4. Apply max output limits.
5. Record execution in manifest if enabled.
6. Do not silently execute untrusted templates.

### File access

This version uses `--cwd` but does not sandbox.

Rules:

1. Relative paths resolve from `--cwd`.
2. Absolute paths are allowed.
3. Symlinks are followed according to OS defaults.
4. No path confinement is promised.
5. Future versions may add sandboxing, but not this version.

### Binary files

Default:

1. Refuse binary file inclusion.
2. Failure case: `binary`.
3. Error code: `binary_file_refused`.

No base64 mode in MVP.

### Secrets

MVP does not implement redaction.

Do not claim rendered output is safe for public sharing.

Future reserved options:

```txt
--redact
--fail-on-secret
```

---

## 12. Determinism Requirements

The renderer should be deterministic under stable inputs:

```txt
template content
stdin content
file contents
environment variables
command outputs
cwd
CLI options
selected shell
```

Requirements:

1. Repeated `stdin:` returns cached same content.
2. Source order in manifest follows template order.
3. Same file and same modifiers produce same result.
4. No timestamps or random values unless introduced by `exec:`.
5. Diagnostics include template line/column.

---

## 13. Encoding and Newline Handling

### Encoding

MVP:

1. Template files are UTF-8.
2. Text files are UTF-8.
3. Invalid UTF-8 is failure case `encoding`.
4. Byte truncation must preserve valid UTF-8.

### Newlines

Rules:

1. Preserve template text exactly outside interpolation.
2. Preserve source text unless modifiers transform it.
3. `oneline:` intentionally normalizes whitespace.
4. `indent:` preserves line breaks.
5. Output file should preserve rendered text exactly.
6. Do not convert LF/CRLF unless behavior is explicitly documented.

---

## 14. Examples

### Basic context

Template:

```md
# Context

## User Request

${{stdin:}}

## README

${{file: README.md |> head: 80}}
```

Command:

```bash
cat request.md | <this-cmd> render -t context.tpl.md -o context.md
```

### File chunk

```md
## Source

${{file: src/app.ts |> lines: 40-120 |> indent: 2}}
```

### Command output

```md
## Git Status

${{exec: git status --short |> timeout: 5 |> stdout: |> max-lines: 100}}
```

Command:

```bash
<this-cmd> render -t context.tpl.md --allow-exec
```

If `--allow-exec` is omitted, rendering fails with exec disabled.

### Shell pipe inside exec

```md
## Diff

${{exec: git diff -- src/app.ts | head -300 |> timeout: 10 |> stdout:}}
```

`|` belongs to the shell command. `|>` separates renderer modifiers.

### Missing file with fallback

```txt
${{file: notes.md |> on-404: ""}}
```

If `notes.md` is missing, render empty string.

### Command failure with fallback

```txt
version=${{exec: git describe --tags |> timeout: 5 |> on-error: "unknown" |> on-timeout: "unknown"}}
```

### One-line value

```txt
branch=${{exec: git rev-parse --abbrev-ref HEAD |> trim: |> oneline:}}
```

### Stderr selection

```txt
${{exec: npm test |> timeout: 120 |> stderr: |> tail: 80}}
```

---

## 15. MVP Scope

### Sources

Implement:

```txt
stdin:
file:
exec:
```

Recommended MVP or P1:

```txt
env:
```

Do not implement:

```txt
glob:
```

Do not implement any source that expands into multiple sources.

### Modifiers

Implement:

```txt
lines:
slice:
head:
head-char:
tail:
tail-char:
trim:
oneline:
indent:
max-lines:
max-bytes:
stdout:
stderr:
fallback:
on-error:
on-404:
timeout:
```

Recommended case-specific policies:

```txt
on-timeout:
on-range:
on-binary:
on-encoding:
on-limit:
on-modifier:
```

Parser should accept `on-<case>:` for known cases.

### CLI options

Implement:

```txt
render
-t, --template-file <path>
--template <string>
-o, --output <path>
--overwrite
--cwd <path>
--print compact|json
--allow-exec
--shell auto|sh|bash|pwsh|powershell|cmd
--timeout <seconds>
--total-timeout <seconds>
--max-lines <number>
--max-bytes <number>
--max-file-bytes <number>
--max-command-bytes <number>
--fail-on-truncated
--manifest <path>
--check
--list-sources
--no-color
```

---

## 16. Parser Requirements

### Block parsing

The parser must find unescaped `${{` and matching unescaped `}}`.

Invalid cases:

```txt
${{file README.md}}
${{file: README.md |>}}
${{file: README.md |> unknown: x}}
${{file: README.md
```

### Command parsing

Every source and modifier uses:

```txt
name:
name: body
```

Examples:

```txt
trim:
stdout:
file: README.md
lines: 1-END
fallback: "not found"
```

Reject old forms:

```txt
{{file: README.md}}
@{{README.md}}
!{{git status}}
${{stdin}}
${{file README.md}}
```

Rationale: required colon makes source/modifier recognition unambiguous and uniform.

### Source position

First command in expression must be a source.

Valid:

```txt
${{file: README.md |> head: 10}}
```

Invalid:

```txt
${{head: 10 |> file: README.md}}
```

Error:

```txt
first_command_must_be_source
```

### Unknown command

Unknown source or modifier fails.

Future worker support may handle unknown sources, but not MVP.

---

## 17. Source Location Tracking

For every interpolation, track:

```txt
template path
line
column
raw interpolation text
source command
modifiers
```

This enables agents to repair invalid templates.

---

## 18. Implementation Architecture

Suggested modules:

```txt
cli
template_loader
template_parser
expression_parser
source_resolver
modifier_engine
exec_runner
diagnostics
manifest_writer
safe_writer
```

Flow:

```txt
parse CLI
resolve --cwd
load template
parse template into literal segments + interpolation segments
if --check: validate and exit
if --list-sources: print sources and exit
cache stdin if needed
for each interpolation:
  parse source + modifiers
  separate transform modifiers and policy modifiers
  resolve source
  apply transform modifiers left-to-right
  apply limits
  if failure occurs:
    try on-<case>
    else try fallback
    else fail render
join rendered segments
write stdout or atomic output file
write manifest if requested
```

---

## 19. Test Plan

### Parser tests

1. Parses `${{stdin:}}`.
2. Parses `${{file: README.md}}`.
3. Parses `${{file: src/app.ts |> lines: 1-END |> indent: 2}}`.
4. Allows `|` inside `exec:` command.
5. Separates modifiers on `|>`.
6. Parses quoted body containing `|>`.
7. Rejects `{{...}}`.
8. Rejects `@{{...}}`.
9. Rejects missing colon.
10. Rejects nested interpolation.
11. Tracks line/column.

### Source tests

1. `stdin:` reads piped input.
2. repeated `stdin:` returns same content.
3. `file:` reads UTF-8 file.
4. missing file is `404`.
5. missing file with `on-404:` renders fallback.
6. binary file is `binary`.
7. `exec:` fails without `--allow-exec`.
8. `exec:` succeeds with `--allow-exec`.
9. `exec:` non-zero is `error`.
10. `exec:` timeout is `timeout`.
11. `env:` missing is `404`.

### Modifier tests

1. `lines: 1-3`.
2. `lines: BEG-3`.
3. `lines: 3-END`.
4. `slice: BEG-10`.
5. `head: 10`.
6. `head-char: 10`.
7. `tail: 10`.
8. `tail-char: 10`.
9. `trim:`.
10. `oneline:`.
11. `indent: 2`.
12. `max-lines: 2` truncates.
13. `max-bytes: 10` preserves UTF-8.
14. `stdout:` selects stdout.
15. `stderr:` selects stderr.
16. `fallback:` catches generic recoverable failure.
17. `on-error:` beats `fallback:`.
18. `on-404:` beats `fallback:`.
19. `timeout:` overrides CLI timeout.

### Output tests

1. stdout render works.
2. `-o` writes file.
3. existing output fails without `--overwrite`.
4. existing output succeeds with `--overwrite`.
5. failed render does not leave partial output.
6. `--print json` emits JSON error.
7. `--manifest` writes valid JSON.
8. `--cwd` changes relative file resolution and exec cwd.

---

## 20. Golden Test Fixtures

Recommended fixture tree:

```txt
fixtures/
  simple.txt
  multiline.txt
  unicode.txt
  crlf.txt
  binary.bin
  template-basic.md
  template-file-lines.md
  template-exec.md
  template-fallback.md
```

Use exact output snapshots.

---

## 21. Acceptance Criteria

MVP is complete when:

1. `${{...}}` syntax works and old `{{...}}`, `@{{...}}`, `!{{...}}` are rejected.
2. All commands use `name:` syntax.
3. `stdin:`, `file:`, and `exec:` work.
4. `exec:` requires `--allow-exec`.
5. `--cwd` controls relative file paths and command cwd.
6. `--print compact|json` controls diagnostic format.
7. `--timeout` defaults to 30 seconds and is overridden by `timeout:`.
8. `lines:` and `slice:` support `<number>`, `BEG`, and `END`.
9. Required modifiers work: `head`, `head-char`, `tail`, `tail-char`, `trim`, `oneline`, `indent`, `max-lines`, `max-bytes`, `stdout`, `stderr`, `fallback`, `on-error`, `on-404`, `timeout`.
10. Missing file can be recovered by `on-404:` or `fallback:`.
11. Exec non-zero can be recovered by `on-error:` or `fallback:`.
12. Output file writes are atomic.
13. Manifest records all sources when `--manifest` is used.
14. Test suite includes parser, source, modifier, output, and golden tests.

---

## 22. Design Rationale

### Why no glob

`glob:` changes the model from:

```txt
one interpolation -> one source -> one string
```

to:

```txt
one interpolation -> many sources -> ordering + formatting + partial failures + output explosion
```

This version intentionally avoids sequence-producing sources.

Agents can use external commands if needed:

```txt
${{exec: find src -name "*.ts" |> stdout: |> max-lines: 100}}
```

This is explicit and guarded by `--allow-exec`.

### Why source/modifier both use `name: body`

Uniform syntax makes parsing and generation easier.

Good:

```txt
${{file: README.md |> head: 20 |> fallback: ""}}
```

Avoid:

```txt
${{file README.md | head(20) || ""}}
```

The colon form is easy to parse, easy to lint, easy for agents to generate, and extensible.

### Why `${{...}}`

It avoids common `{{...}}` template collisions while remaining compact.

### Why `--cwd`, not `--root`

`--cwd` means:

```txt
resolve relative paths here
run commands here
```

`--root` implies sandboxing and security boundaries. This spec does not provide sandbox guarantees.

---

## 23. Future Extensions

Potential future sources:

```txt
worker:
json:
yaml:
toml:
search:
chunk:
git:
redact:
```

Potential future options:

```txt
--worker <command>
--redact
--fail-on-secret
--replay <manifest>
--cache-dir <path>
--allow-binary
```

If sequence sources are introduced later, they should be explicit and bounded, for example:

```txt
${{seq: files "src/**/*.ts" |> limit: 20 |> join: "\n"}}
```

This is intentionally excluded from the current version.

---

## 24. Minimal Example

Template:

```md
# Context

## Request

${{stdin:}}

## Source

${{file: example.txt |> lines: BEG-2 |> indent: 2}}

## Git

${{exec: git status --short |> timeout: 5 |> stdout: |> on-error: "git unavailable"}}
```

File `example.txt`:

```txt
alpha
beta
gamma
```

Command:

```bash
echo "Fix this" | <this-cmd> render -t template.md --allow-exec -o context.md
```

Rendered output shape:

```md
# Context

## Request

Fix this

## Source

  alpha
  beta

## Git

...
```

---

## 25. Final Positioning

`<this-cmd>` is a deterministic context composer for local agents.

It should optimize for:

```txt
explicit typed interpolation
cross-platform execution
bounded output
recoverable failures
clear diagnostics
simple parser
agent-friendly generation
```

It should avoid:

```txt
ambiguous template syntax
sequence expansion in MVP
implicit command execution
full programming language features
silent partial output files
security claims it does not enforce
```
