---
revision: 2
date: 2026-06-10T15:12:44
trigger: "review-feedback"
---

<!-- MUST set trigger to one of: review-feedback | discovery | scope-expansion | correction
This file records scope/design changes after Plan begins (spec/design locked).
Do NOT use revisions during Design phase — edit spec.md/design.md directly.
File naming: revisions/NNN-description.md (incrementing number). -->

# exec spill artifacts and timeout schema

## Reason
<!-- The "cause" in the causal chain: trigger source, what problem or new requirement was discovered. -->

Review feedback identified two reliability gaps in the accepted `compose` implementation:

1. `exec:` captured stdout/stderr with OS pipes but waited for process exit before reading them. A command that writes more than the pipe buffer can block in `write()`, making a healthy command look like a timeout.
2. Byte limits bounded rendered text only after full file/command capture. For large `exec:` output, this risks high memory use and gives agents no path to inspect omitted output.

The user accepted an amended output contract: large `exec:` streams are drained continuously, the rendered body keeps only the configured prefix, excess output is preserved in bounded temp spill artifacts, and JSON output exposes enough metadata for agents to find those artifacts.

## Changes

### Spec Impact
<!-- Which parts of spec.md logically changed?
Do NOT modify the original spec.md — record the changes here. -->

No template syntax changes. The command contract is extended:

- `exec:` output MUST be drained while the child is running; implementation MUST NOT wait for child exit before reading stdout/stderr pipes.
- `exec:` streams MUST NOT be killed only because they exceed `--max-command-bytes`; timeout remains the reason to terminate a running command.
- When an `exec:` stream exceeds `--max-command-bytes`, the rendered value uses the first `--max-command-bytes` bytes plus a truncation marker that points to a temp spill artifact.
- Add `--max-spill-bytes <N>`, default `134217728` bytes. This is a per-compose-run total spill budget shared by all `exec:` streams.
- If the shared spill budget is exhausted, the implementation continues draining stdout/stderr to avoid deadlock, discards further excess bytes, and marks the artifact `complete: false`.
- With `--fail-on-truncated`, the command still preserves spill artifacts where possible and returns a structured `limit` error containing those artifacts.
- `--total-timeout` means total render-phase wall-clock budget for all interpolations. A single `exec:` effective timeout is `min(local exec timeout, remaining total render budget)`.
- Compose JSON envelopes MUST include `meta.schemaVersion = 1` and `meta.cwd`.
- Compose success and error payloads MAY include `artifacts`, currently for `exec:` spill files.
- Template-load failures in JSON mode MUST use the compose error envelope instead of falling through to top-level plain `anyhow` output.

### Design Impact
<!-- Which parts of design.md changed? Delete this section if no design.md exists. -->

#### CLI

```text
asq compose ... [--max-spill-bytes <N>]

Defaults:
  --max-command-bytes 1048576
  --max-spill-bytes   134217728
```

#### Render deadline

```text
render_start = Instant::now()
render_deadline = render_start + --total-timeout, if provided

for each interpolation:
  remaining_total = render_deadline - now
  exec_timeout = min(local_timeout_or_default, remaining_total)
  resolve/eval interpolation under exec_timeout
```

`--total-timeout` applies to render/eval work only. It does not apply to `--check`, `--list-sources`, or final body write.

#### Exec capture

```text
spawn child with stdout/stderr piped
  → concurrently drain stdout and stderr while child runs
  → keep prefix bytes up to --max-command-bytes for render selection
  → once stream exceeds --max-command-bytes:
      create spill file under temp agent-temp
      write prefix + excess bytes to spill while shared spill budget remains
      keep draining after budget exhaustion, discarding drained bytes
  → on timeout: kill child, wait, join drain workers, return timeout error
  → on non-zero exit: return error, preserving spill artifacts if produced
  → on success: decode prefix for rendering and attach stream artifacts
```

Size limits do not terminate the child. Timeout remains the execution kill boundary.

#### Truncation marker

Rendered text for a selected truncated stream MUST include a marker after the decoded prefix:

```text
[asq compose: stdout truncated after 1048576 bytes; spill saved to C:\...\asq-compose-spill-...stdout.txt]
```

If spill budget was exhausted, the marker MUST state that the spill is capped/incomplete.

#### JSON schema

```ts
type ComposeCommand = "compose";

interface ComposeMeta {
  schemaVersion: 1;
  cwd: string;
}

interface ComposeSuccessEnvelope<T> {
  ok: true;
  command: ComposeCommand;
  data: T;
  warnings: string[];
  meta: ComposeMeta;
}

interface ComposeErrorEnvelope {
  ok: false;
  command: ComposeCommand;
  error: ComposeError;
  warnings: string[];
  meta: ComposeMeta;
}

interface Location {
  line: number;
  column: number;
}

type FailureCase =
  | "404"
  | "error"
  | "timeout"
  | "range"
  | "encoding"
  | "binary"
  | "limit"
  | "modifier"
  | "parse";

interface ComposeError {
  code: string;
  case?: FailureCase;
  message: string;
  raw?: string;
  location?: Location;
  artifacts?: ComposeArtifact[];
}

interface OutputInfo {
  kind: "temp" | "file";
  path: string;
}

interface ComposeStatus {
  output?: OutputInfo;
  bytes: number;
  sources: number;
  truncated: boolean;
  artifacts?: ComposeArtifact[];
}

interface SourceListData {
  sources: SourceInfo[];
}

interface CheckData {
  valid: true;
  sources: number;
}

interface SourceInfo {
  index: number;
  kind: "stdin" | "file" | "env" | "exec";
  argument: string;
  location: Location;
}

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

#### Module boundary updates

```text
model.rs    add ComposeArtifact and artifact-bearing ComposeError/ComposeStatus
render.rs   own render deadline and aggregate artifacts
sources.rs  own concurrent exec drain, spill budget, spill artifact creation
output.rs   own compose JSON meta shape for success/check/error
mod.rs      expose --max-spill-bytes and route template-load errors through compose errors
```

### Task Impact
<!-- Impact on tasks.md: which tasks were added/modified/removed.
tasks.md is a living document — update it directly. -->

Add feedback tasks to `tasks.md`:

- Add artifact/error/status data model and JSON meta schema.
- Add `--max-spill-bytes` with 128MiB per-run budget.
- Refactor `exec:` capture to drain stdout/stderr concurrently, spill excess output, and avoid pipe deadlock.
- Enforce `--total-timeout` as a render-wide deadline and pass remaining budget to exec resolution.
- Route template-load failures through compose structured errors.
- Update README/CHANGELOG and targeted tests.
