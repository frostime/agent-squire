pub(crate) const COMPOSE_PROMPT: &str = r#"# Squire compose template guide

`asq compose` renders agent context templates into bounded UTF-8 output files.
Prefer `--template-file` for non-trivial templates; use `--template` only for tiny one-liners because shell quoting differs across platforms.

## Workflow

```bash
asq compose -t context.tpl.md --check
asq compose -t context.tpl.md --list-sources
asq compose -t context.tpl.md
```

Default output is a persistent file under the system temp `agent-temp` directory, and compact stdout reports its path:

```text
output: C:\Users\...\Temp\agent-temp\asq-compose-<timestamp>-<uuid>.md
```

Read that file for the rendered body. Use `--stdout` only when the body should be piped to another command.
In JSON mode, status output reports `data.output.path` and does not embed the rendered body.

## Source Decision Table

| Need | Template |
|---|---|
| Current piped/user text | `${{stdin |> trim}}` |
| Whole file or file slice | `${{file: README.md |> lines: 1-80}}` |
| Environment variable | `${{env: NAME |> fallback: ""}}` |
| Command stdout | `${{exec: git status --short |> timeout: 5 |> stdout}}` |
| Command stderr | `${{exec: cargo test |> timeout: 120 |> stderr}}` |
| Recover missing/failed source | `${{file: optional.md |> on-404: ""}}` |

## Syntax

```md
${{source |> stage |> stage}}
```

Source commands appear first: `stdin`, `file: path`, `env: NAME`, `exec: command`.
No-argument commands may omit the colon: `stdin`, `trim`, `stdout`, `stderr`.
Commands with bodies use `name: body`.

Stage command roles:

- runtime: `timeout: 5`
- stream: `stdout`, `stderr` (`exec` only)
- transform: `lines: 1-END`, `head: 80`, `tail: 40`, `trim`, `indent: 2`, `max-bytes: 4096`
- policy: `fallback: ""`, `on-404: "missing"`, `on-error: "failed"`, `on-timeout: "timed out"`

Text transforms run left-to-right. Runtime controls, stream selectors, and failure policies are normalized by role.

## Recipes

Include a bounded README excerpt:

```md
## README

${{
  file: README.md
  |> lines: 1-120
  |> indent: 2
}}
```

Include stdin and trim surrounding whitespace:

```md
## Request

${{stdin |> trim}}
```

Include command output safely:

```md
## Git Status

${{exec: git status --short |> timeout: 5 |> stdout |> max-lines: 100 |> on-error: "git status unavailable"}}
```

Use a literal multiline fallback with JSON string escapes:

```md
${{file: missing.md |> fallback: "line1\nline2"}}
```

## Safety And Limits

`exec:` is disabled unless `--allow-exec` is passed. `stdin` fails when stdin is an interactive terminal, preventing accidental hangs.

`--total-timeout` is the whole render-phase wall-clock budget.
A local `timeout:` stage limits one `exec`; the effective exec timeout is the smaller of local timeout and remaining total timeout.

Large `exec:` stdout/stderr streams are drained while the child runs.
Rendered text keeps the first `--max-command-bytes` per stream; excess bytes spill to temp files under the per-render `--max-spill-bytes` budget.
Size truncation does not kill `exec`; timeout does.

## Debugging

- Use `--check` to catch syntax and static modifier errors without reading files or running commands.
- Use `--list-sources` to inspect discovered sources without resolving them.
- If output contains `[asq compose: ... saved to PATH]`, read `PATH` for the spilled full stream.
- In JSON mode, inspect `data.artifacts` on success and `error.artifacts` on failure.
- Use `--fail-on-truncated` when truncation should fail the run but spill artifacts should still be preserved.
"#;
