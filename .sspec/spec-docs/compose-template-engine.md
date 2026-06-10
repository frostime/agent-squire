---
name: Compose Template Engine
description: Parse/compile/render contracts, source semantics, exec spill artifacts, and JSON output schema for the compose command.
updated: 2026-06-10
scope:
  - /src/builtins/compose/**
  - /tests/compose.rs
  - /README.md
---

# Compose Template Engine

## Overview

The `compose` built-in in [`/src/builtins/compose/mod.rs`](/src/builtins/compose/mod.rs) renders context templates into UTF-8 Markdown-like output for agents. A template combines literal text with interpolation blocks:

```md
${{source |> stage |> stage}}
```

The implementation is intentionally split into side-effect-free phases and side-effectful phases. Maintain this split so `--check` and `--list-sources` remain safe for untrusted templates.

```text
load template text
  -> parse      no side effects  /src/builtins/compose/parser.rs
  -> compile    no side effects  /src/builtins/compose/compile.rs
  -> render     reads/runs       /src/builtins/compose/render.rs
  -> write      writes output    /src/builtins/compose/output.rs
```

## Phase Model

| Phase | Owner | Input | Output | Side effects |
|---|---|---|---|---|
| Parse | [`parser.rs`](/src/builtins/compose/parser.rs) | template text | `Template` AST | No |
| Compile | [`compile.rs`](/src/builtins/compose/compile.rs) | `Template` | `CompiledTemplate` | No |
| Render/eval | [`render.rs`](/src/builtins/compose/render.rs) | `CompiledTemplate` | rendered text + artifacts | Yes: source reads, exec |
| Source resolution | [`sources.rs`](/src/builtins/compose/sources.rs) | `SourceSpec` | `ResolvedSource` | Yes: stdin/file/env/exec |
| Transform | [`modifiers.rs`](/src/builtins/compose/modifiers.rs) | selected text | transformed text | No |
| Output | [`output.rs`](/src/builtins/compose/output.rs) | rendered text/status | stdout/file/json | Yes: stdout/file/stderr |

Phase invariants:

- `--check` executes load -> parse -> compile, then exits.
- `--list-sources` executes load -> parse -> compile, prints compiled `SourceInfo`, then exits.
- Compile owns static semantic validation and normalization.
- Render is the first phase allowed to evaluate `stdin`, `file`, `env`, or `exec` sources.

## Template Semantics

[`parser.rs`](/src/builtins/compose/parser.rs) treats every interpolation as one source command followed by zero or more stage commands. [`compile.rs`](/src/builtins/compose/compile.rs) classifies stage commands by role:

| Role | Commands | Execution rule |
|---|---|---|
| Source | `stdin`, `file`, `env`, `exec` | Exactly one, first command only |
| Runtime control | `timeout` | Applies during source resolution |
| Stream selector | `stdout`, `stderr` | Selects an `exec` stream before text transforms |
| Text transform | `lines`, `slice`, `head`, `head-char`, `tail`, `tail-char`, `trim`, `oneline`, `indent`, `max-lines`, `max-bytes` | Applies left-to-right in written order |
| Failure policy | `fallback`, `on-404`, `on-error`, `on-timeout`, `on-range`, `on-binary`, `on-encoding`, `on-limit`, `on-modifier` | Recovers matching failures |

Do not reorder text transforms. These two templates have different meanings:

```md
${{file: log.txt |> tail: 100 |> head: 10}}
${{file: log.txt |> head: 10 |> tail: 100}}
```

Role normalization is allowed for non-transform stages. This template:

```md
${{exec: npm test |> head: 80 |> stderr |> timeout: 120 |> on-error: "failed"}}
```

normalizes to:

```text
source     = exec: npm test
runtime    = timeout: 120
stream     = stderr
transforms = [head: 80]
policies   = { on-error: "failed" }
```

Compile-time compatibility rules:

- A stream selector is valid only when the source is `exec`.
- `stdout` and `stderr` are mutually exclusive.
- Duplicate `timeout` is invalid.
- Duplicate fallback policy for the same case is invalid.
- Parse failures cannot be recovered by fallback inside the same invalid interpolation.

## Source Resolution

[`sources.rs`](/src/builtins/compose/sources.rs) resolves sources under the effective `cwd` from [`/src/cli.rs`](/src/cli.rs).

| Source | Behavior |
|---|---|
| `stdin` | Read process stdin once and cache it. Fail fast if stdin is an interactive terminal. |
| `file` | Resolve relative path against `cwd`, read one file, decode text, reject binary/null bytes. |
| `env` | Read one environment variable. Missing variable is failure case `404`. |
| `exec` | Disabled unless `--allow-exec` is set. Run through the selected shell in `cwd`. Capture stdout/stderr. |

Template sources are distinct from runtime argument sources in [`/src/runtime/input.rs`](/src/runtime/input.rs). Keep `${{stdin}}`, `${{file: ...}}`, and `${{env: ...}}` separate from `@stdin`, `@file:path`, and `@env:NAME` CLI argument syntax.

Text decoding uses [`text.rs`](/src/builtins/compose/text.rs): UTF-8 BOM, UTF-8, GBK, Windows-1252 fallback, then lossy UTF-8 fallback where applicable. Output files are UTF-8 without BOM.

## Exec Spill Artifacts

`exec` is the main trust and resource boundary. It must be safe for commands that produce more output than an OS pipe buffer.

Execution model in [`sources.rs`](/src/builtins/compose/sources.rs):

```text
spawn child with stdout/stderr piped
  -> drain stdout and stderr concurrently while child runs
  -> retain at most --max-command-bytes per stream for rendered text
  -> when a stream exceeds --max-command-bytes:
       create a temp spill file under system temp / agent-temp
       write retained prefix + excess bytes while shared spill budget remains
       after budget exhaustion, keep draining and discard further bytes
  -> if timeout expires, kill child and preserve produced artifacts
  -> if child exits non-zero, return error and preserve produced artifacts
```

Resource contracts:

| Limit | Meaning |
|---|---|
| `--max-command-bytes` | Per-stream rendered prefix budget for `exec` output. |
| `--max-spill-bytes` | Per-compose-run total spill file budget shared by all `exec` streams. Default: `134217728` bytes. |
| `--timeout` / `timeout:` | Per-exec timeout. |
| `--total-timeout` | Render-phase wall-clock budget shared by all interpolations. |

Size limits do not kill the child. Timeout kills the child. When spill budget is exhausted, keep draining to avoid deadlock and mark the artifact `complete: false`.

Spill markers are appended after text transforms and global limits in [`render.rs`](/src/builtins/compose/render.rs). Keep this ordering so transforms such as `head`, `lines`, and `max-bytes` cannot remove the marker:

```text
selected stream prefix
  -> text transforms
  -> global limits
  -> append selected spill marker
```

## Failure And Recovery

Failure cases are defined in [`model.rs`](/src/builtins/compose/model.rs) as `FailureCase` values:

```text
404, error, timeout, range, encoding, binary, limit, modifier, parse
```

Failure policy resolution:

```text
failure occurs
  -> matching on-<case> policy exists? use that literal
  -> fallback policy exists? use fallback literal
  -> fail whole render
```

Recovery happens per interpolation in [`render.rs`](/src/builtins/compose/render.rs). Recovered fallback text is a literal and is not fed back through remaining transforms.

`--fail-on-truncated` turns truncation into a `limit` error. Preserve spill artifacts in that error so agents can inspect temp files even when no rendered body is produced.

## Output Contract

[`output.rs`](/src/builtins/compose/output.rs) owns output target behavior.

| Mode | Rendered body target | Status/diagnostics |
|---|---|---|
| default | temp file under system temp `agent-temp` | stdout reports `output: <path>` |
| `--output <path>` | explicit file | stdout reports `output: <path>` |
| `--stdout` | stdout | diagnostics/errors on stderr; no success status |
| `--print json` with file target | temp/file | stdout JSON envelope; rendered body is not embedded |
| `--print json` with error | none | stderr JSON error envelope |

JSON meta contract:

```ts
interface ComposeMeta {
  schemaVersion: 1;
  cwd: string;
}
```

Status data contract:

```ts
interface ComposeStatus {
  output?: { kind: "temp" | "file"; path: string };
  bytes: number;
  sources: number;
  truncated: boolean;
  artifacts?: ComposeArtifact[];
}
```

Error data contract:

```ts
interface ComposeError {
  code: string;
  case?: "404" | "error" | "timeout" | "range" | "encoding" | "binary" | "limit" | "modifier" | "parse";
  message: string;
  raw?: string;
  location?: { line: number; column: number };
  artifacts?: ComposeArtifact[];
}
```

Artifact contract:

```ts
interface ComposeArtifact {
  kind: "spill";
  path: string;
  sourceIndex: number;
  sourceKind: "exec";
  stream: "stdout" | "stderr";
  renderedBytes: number;
  savedBytes: number;
  maxSavedBytes: number;
  complete: boolean;
  message: string;
}
```

Do not embed rendered body text in JSON status. Agents should read the `output.path` file when they need the body.

## Maintainer Invariants

Maintain these invariants when editing code covered by this document:

- Keep parse and compile side-effect free.
- Keep source listing derived from the compiled program, not source evaluation.
- Keep stream selectors valid only for `exec` sources.
- Keep text transforms left-to-right in author-written order.
- Keep runtime controls, stream selectors, and failure policies role-normalized outside the transform list.
- Keep spill marker append after transforms/global limits.
- Keep size limits distinct from timeout: size truncates/spills; timeout kills.
- Keep `--stdout` success path body-only on stdout.
- Keep JSON status free of rendered body text.
- Update this document when changing `src/builtins/compose/**` contracts.
