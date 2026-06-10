---
change: "compose"
created: 2026-06-10T01:22:27
---

# Design: compose

<!-- MUST maintain quality bar (non-negotiable):
Use semi-structured, formalized expression over flat prose.
Goal: maximize information density, minimize ambiguity, optimize reader comprehension.
In short: show, don't describe.

Fence nesting: when showing content that contains ```, outer fence MUST use more backticks. Always outer > inner.

Recommended tools (non-exhaustive):
- typed code block: interfaces, types, schemas, config, prompts...
- ASCII diagram: call chains, state machines, module trees, content outlines...
- table: before/after comparison, option tradeoffs, scope mapping...
- labeled items: multi-change annotation (Fix A / Feat B / Step 1...)
- pseudocode, decision trees, constraint lists

Anti-pattern:
  ❌ "We will add a function that accepts X and returns Y"
  ✅ `def process(x: Input) -> Output: ...`

  ❌ "The request first goes through module A, then is passed to B"
  ✅ request → A.validate() → B.process() → response
-->

<!-- SHOULD organize by the nature of the change. No fixed sections required.
Reference patterns by change type (pick what fits, not mandatory):

Feature/Bugfix  → interface signatures + behavioral flow + data model
Refactor        → before/after structural comparison + migration steps
Docs/Templates  → content outline + section hierarchy
Prompt/Rules    → before/after examples + decision logic
Config/Schema   → schema definition + migration path + compatibility strategy
-->

## Interface Contract

```text
asq compose (--template-file <path> | --template <text>) [options]
asq compose -t <path> [options]

Template input:
  -t, --template-file <path>   Read template from file path
      --template <text>        Treat argument as literal template text

Output target:
      --stdout                 Write rendered body to stdout
  -o, --output <path>          Write rendered body to explicit file path
      --overwrite              Allow replacing an existing --output file

Execution:
      --allow-exec             Enable exec: sources
      --shell <mode>           auto | sh | bash | pwsh | powershell | cmd
      --timeout <seconds>      Default dynamic-source timeout; default 30
      --total-timeout <sec>    Optional whole-render timeout

Limits:
      --max-lines <n>
      --max-bytes <n>
      --max-file-bytes <n>     Default 1048576
      --max-command-bytes <n>  Default 1048576
      --fail-on-truncated

Inspection:
      --check                  Parse/validate template without resolving sources
      --list-sources           Print discovered sources without reading/running them
      --prompt                 Print an agent-facing long usage guide and exit
```

Mutual exclusion:

| Option set | Behavior |
|------------|----------|
| no `--stdout`, no `--output` | Render to `%TEMP%/agent-temp/asq-compose-<timestamp>-<uuid>.md`; stdout reports path/status. |
| `--stdout` | Rendered body goes to stdout; diagnostics/errors go to stderr. |
| `--output <path>` | Rendered body goes to path; fails if path exists unless `--overwrite`. |
| `--stdout` + `--output` | CLI usage error. |
| `--template-file` + `--template` | CLI usage error. |

## Behavioral Flow

```text
parse CLI
  → resolve effective cwd from global CommandContext
  → if --prompt: print embedded agent guide and exit
  → load template via宽松 text decoder
  → parse template into Segment::Literal / Segment::Interpolation
  → if --check: validate command names/bodies and exit
  → if --list-sources: print source table/json and exit
  → if template contains stdin: read stdin once; fail if terminal
  → for each interpolation in order:
      parse expression
      split source + modifiers + fallback policies
      resolve source
      apply transforms/limits/stream selectors
      recover with on-<case> / fallback when possible
  → join rendered segments
  → write body to temp/output/stdout target
  → print compact/json status when body target is a file
```

## Agent Prompt

`--prompt` prints a concise built-in guide for agents, similar to `asq patch --prompt`. It is documentation output, not render output.

Guide outline:

```text
# Squire compose template guide

## Command
- default temp output
- --stdout pipeline mode
- --template-file / --template
- --allow-exec trust boundary

## Syntax
- source first
- no-arg commands may omit colon
- stage command roles
- multiline interpolation
- JSON string bodies

## Examples
- stdin + file context
- exec with stderr/head/timeout
- fallback patterns
- output modes
```

Rules:

| Rule | Behavior |
|------|----------|
| Side effects | `--prompt` does not read templates, stdin, files, env vars, or run exec. |
| Output channel | Guide prints to stdout. |
| Print mode | `--prompt` ignores `--print`; it is always readable Markdown/plain text. |
| Compatibility | Mirrors `patch-edit` prompt style without changing global help. |

## Template Grammar

### Lexical Form

Every interpolation is one expression. Every expression starts with exactly one source command, followed by zero or more stage commands.

```text
Interpolation = "${{" WS Expression WS "}}"
Expression    = SourceCommand (WS "|>" WS StageCommand)*
Command       = Name | Name ":" WS Body?
Name          = [A-Za-z_][A-Za-z0-9_-]*
Body          = JsonString | UnquotedBody
```

Commands with bodies use `name: body`. Commands with no arguments may omit the colon; `stdin`, `stdin:`, `trim`, and `trim:` are equivalent. Command *roles* are distinct even though lexical form is shared.

| Role | Position | Commands | Meaning |
|------|----------|----------|---------|
| Source | First command only | `stdin`, `file`, `env`, `exec` | Produces the initial value. |
| Runtime control | After source | `timeout` | Configures dynamic source resolution. |
| Stream selector | After source | `stdout`, `stderr` | Selects an `exec:` output stream before text transforms. |
| Text transform | After source | `lines`, `slice`, `head`, `head-char`, `tail`, `tail-char`, `trim`, `oneline`, `indent`, `max-lines`, `max-bytes` | Transforms the selected text value. |
| Failure policy | After source | `fallback`, `on-404`, `on-error`, `on-timeout`, `on-range`, `on-binary`, `on-encoding`, `on-limit`, `on-modifier` | Provides recovery text for recoverable failures. |

No-argument commands: `stdin`, `trim`, `oneline`, `stdout`, and `stderr` may be written without `:`. Body-taking commands must include `:`; empty fallback text should be explicit JSON string `fallback: ""`.

### Semantic Normalization

Stage commands may be written in any order after the source, but they do not all execute left-to-right. The parser classifies commands by role and builds this semantic expression:

```rust
struct Expression {
    source: SourceCommand,
    runtime: RuntimeControls,
    stream: Option<StreamSelector>,
    transforms: Vec<TextTransform>,
    policies: FailurePolicies,
}
```

Execution order is fixed:

```text
source
  → runtime controls apply during source resolution
  → stream selector applies to exec output
  → text transforms apply left-to-right in written order
  → failure policies recover matching failures
```

Example normalization:

```md
${{exec: npm test |> head: 80 |> stderr |> timeout: 120 |> on-error: "failed"}}
```

is interpreted as:

```text
source     = exec: npm test
runtime    = timeout: 120
stream     = stderr
transforms = [head: 80]
policies   = { on-error: "failed" }
```

### Body Rules

| Body form | Rule |
|-----------|------|
| Unquoted body | Read until unquoted `|>` or interpolation close; trim outer whitespace; meaningful internal newline is invalid. |
| JSON string body | JSON unescaping applies; supports `\n`, `\t`, `\"`, `\\`, `\uXXXX`; may contain literal `|>` and `}}`. |
| Empty/no body | Valid only for no-argument commands, such as `stdin`, `stdin:`, `trim`, `trim:`, `stdout`, `stdout:`. |

### Parse And Validation Errors

| Case | Error |
|------|-------|
| First command is not a source | `first_command_must_be_source` |
| Source command appears after first position | `source_after_first` |
| Unknown command name | `unknown_command` |
| Body-taking command has no colon/body | `missing_body` |
| Unclosed interpolation | `unclosed_interpolation` |
| Nested interpolation | `nested_interpolation_not_supported` |
| Meaningful newline in unquoted body | `multiline_unquoted_body` |
| Invalid JSON string body | `invalid_quoted_body` |
| Multiple `timeout:` controls | `duplicate_runtime_modifier` |
| Both `stdout:` and `stderr:` appear | `conflicting_stream_selectors` |
| Duplicate `fallback:` or duplicate same `on-<case>:` | `duplicate_failure_policy` |
| Stream selector on non-`exec:` source | `modifier` failure case |

### Multiline And Escaping

| Area | Rule |
|------|------|
| Multiline template | Template text may contain any newlines; literal text is preserved exactly. |
| Multiline interpolation | `${{ ... }}` may span lines; whitespace around commands/separators is ignored. |
| Multiline body | Use JSON escapes, e.g. `fallback: "line1\nline2"`; raw multiline body is invalid. |
| Escaping literals | `\${{` renders `${{`; `\}}` renders `}}` outside interpolation. |

Readable multiline interpolation:

```md
${{
  file: README.md
  |> lines: 1-END
  |> indent: 2
}}
```

Multiline literal body must use JSON escapes:

```md
${{
  file: missing.md
  |> fallback: "line1\nline2"
}}
```

## Sources

| Source | Body | MVP behavior |
|--------|------|--------------|
| `stdin:` | empty | Read process stdin once and cache; fail with `stdin_unavailable` if stdin is terminal. |
| `file:` | path | Resolve relative path against global `--cwd`; read one text file; reject binary. |
| `env:` | name | Read one environment variable; missing is failure case `404`. |
| `exec:` | command | Disabled unless `--allow-exec`; run through selected shell in global `--cwd`; capture stdout/stderr. |

`@stdin`, `@file:path`, and `@env:NAME` remain ASQ argument-source syntax only. Compose template source syntax is always `${{stdin:}}`, `${{file: ...}}`, `${{env: ...}}`, or `${{exec: ...}}`.

## Stage Command Semantics

### Runtime Controls

| Command | Applies to | Rule |
|---------|------------|------|
| `timeout: <seconds>` | `exec:` | Overrides CLI `--timeout`; positive integer only; duplicate local timeout is invalid. |

Runtime controls are collected before source resolution. They are not text transforms.

### Stream Selectors

| Command | Applies to | Rule |
|---------|------------|------|
| `stdout:` | `exec:` | Select stdout stream; default if no selector exists. |
| `stderr:` | `exec:` | Select stderr stream. |

Stream selection happens before text transforms no matter where the selector is written. `stdout:` and `stderr:` are mutually exclusive. Stream selectors on `stdin:`, `file:`, or `env:` fail with failure case `modifier`.

### Text Transforms

Text transforms are the only commands applied left-to-right in written order.

| Command | Unit | Rule |
|---------|------|------|
| `lines: A-B` | line | Inclusive 1-based range; endpoints support numbers, `BEG`, `END`. |
| `slice: A-B` | Rust `char` | Inclusive 1-based Unicode scalar range; endpoints support numbers, `BEG`, `END`. |
| `head: N` / `tail: N` | line | Keep first/last N lines. |
| `head-char: N` / `tail-char: N` | Rust `char` | Keep first/last N Unicode scalar values. |
| `trim:` | text | Trim leading/trailing whitespace from entire value. |
| `oneline:` | text | Trim, then collapse each whitespace run into one space. |
| `indent: N` | line | Prefix every line, including empty lines, with N spaces. |
| `max-lines: N` | line | Truncate after N lines unless `--fail-on-truncated`. |
| `max-bytes: N` | UTF-8 byte | Truncate without producing invalid UTF-8 unless `--fail-on-truncated`. |

Example where order matters:

```md
${{file: log.txt |> tail: 100 |> trim: |> indent: 2}}
```

### Failure Policies

| Command | Rule |
|---------|------|
| `fallback: <value>` | Recovery text for any recoverable failure without a more specific policy. |
| `on-<case>: <value>` | Recovery text for one failure case; takes precedence over `fallback:`. |

Known failure cases: `404`, `error`, `timeout`, `range`, `encoding`, `binary`, `limit`, `modifier`.

Failure policy resolution:

```text
failure case occurs
  → matching on-<case> exists? render that literal
  → fallback exists? render fallback literal
  → fail whole render
```

Failure policies are collected; they are not transforms and are not affected by written order. Parse failures are not recoverable by fallback inside the same invalid interpolation.

## Encoding And Output

| Item | Behavior |
|------|----------|
| Template file | Decode UTF-8 BOM, UTF-8, GBK, Windows-1252 fallback; reject binary/null bytes. |
| `file:` source | Same宽松 decoding as template file. |
| `stdin:` / `exec:` | Decode as UTF-8 lossless when valid; use宽松 fallback for captured bytes. |
| Output files | Always UTF-8 without BOM. |
| Newlines | Preserve template/source text unless modifiers transform it; no automatic LF/CRLF normalization. |

## Output Status

Compact success when body target is file:

```text
output: C:\Users\ypz\AppData\Local\Temp\agent-temp\asq-compose-20260610T012244-abcd.md
```

JSON success when body target is file:

```json
{
  "ok": true,
  "command": "compose",
  "data": {
    "output": { "kind": "temp", "path": "..." },
    "bytes": 1234,
    "sources": 3,
    "truncated": false
  },
  "warnings": [],
  "meta": { "cwd": "..." }
}
```

`data` MUST NOT contain rendered body text.

## Module Blueprint

```text
src/builtins/compose/
  mod.rs          CLI args + run orchestration
  model.rs        segments, expressions, commands, diagnostics
  parser.rs       template/expression parsing + source locations
  text.rs         decoding, newline, truncation, char/line helpers
  sources.rs      stdin/file/env/exec resolvers
  modifiers.rs    transform engine + fallback policy
  output.rs       temp/output/stdout writer + compact/json status
```
