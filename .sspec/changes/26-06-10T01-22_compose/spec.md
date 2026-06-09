---
name: compose
status: REVIEW
change-type: single
created: 2026-06-10 01:22:27
reference:
- source: .sspec/requests/26-06-10T00-36_composer.md
  type: request
  note: Linked from request
- source: .sspec/changes/26-06-10T01-22_compose/reference/context-composer-prd.md
  type: doc
  note: Seed PRD preserved from clarify request
---
<!-- MUST follow frontmatter schema:
status: PLANNING | DOING | REVIEW | DONE | BLOCKED
change-type: single | sub
reference?: Array<{source, type: 'request'|'root-change'|'sub-change'|'prev-change'|'doc'|'revision', note?}>

Sub-change MUST link root:
reference:
  - source: ".sspec/changes/<root-change-dir>"
    type: "root-change"
    note: "Phase <n>: <phase-name>"

Single-change common reference:
reference:
  - source: ".sspec/requests/<request-file>.md"
    type: "request"
  - source: ".sspec/changes/<change-dir>"
    type: "prev-change"
    note: "Follow-up to <change-name>."
-->

# compose

## Problem Statement

<!-- Quantify impact. Format: "[metric] causing [impact]".
Simple: single paragraph. Complex: split into "Current state" + "User need". -->

Agents currently need ad-hoc shell glue to assemble request context from stdin, files, command output, and environment values, causing fragile cross-platform workflows, oversized terminal output, and repeated manual cleanup when context should instead be rendered into a reusable UTF-8 file.

## Proposed Solution

### Approach
<!-- Core solution (1-3 paragraphs) + why this approach over alternatives -->

Add a new built-in `compose` command that renders deterministic context templates using `${{ source |> modifier }}` interpolation. By default, rendered content is written to `%TEMP%/agent-temp/asq-compose-<timestamp>-<uuid>.md`; stdout reports the output path/status. `--stdout` is the explicit opt-in for pipeline mode where rendered content goes directly to stdout.

The command keeps rendered body separate from diagnostics and JSON. JSON output may report status, errors, source metadata, and output path, but MUST NOT embed the rendered body. This preserves agent-friendly file output while avoiding huge JSON payloads.

The design adopts the PRD's typed interpolation model but adapts it to ASQ: global `--cwd` and `--print` are reused, text input decoding is宽松 for Chinese/English environments, output files are always UTF-8 without BOM, `exec:` is implemented behind `--allow-exec`, and `--manifest` is excluded from MVP.

### Key Change
<!-- MUST label each independent change item as **Type Label: Title**.
Examples: **Fix A: Request linking** / **Feat B: Cache TTL jitter**
tasks.md references these labels — MUST NOT copy the design description.
If scope boundary is unclear, add a "What Stays Unchanged" block after Scope Summary.
Fence nesting: when showing content containing ```, outer fence MUST use more backticks (outer > inner). -->

**Feat A: Compose CLI** — Add `asq compose` / `squire compose` with template input, output target, execution, limit, validation, and source listing options.

**Feat B: Template Parser** — Parse multiline `${{ ... }}` interpolation blocks, JSON quoted bodies, `|>` modifier chains, escaping, source locations, and parse failures.

**Feat C: Source Resolution** — Resolve `stdin:`, `file:`, `env:`, and guarded `exec:` sources using `--cwd`,宽松 text decoding, cached stdin, shell selection, timeout, and binary refusal.

**Feat D: Stage Command Engine** — Classify post-source commands into runtime controls, stream selectors, text transforms, and failure policies; normalize non-transform stages deterministically and apply text transforms left-to-right.

**Feat E: Output Contract** — Write rendered body to Temp by default, explicit output path with `-o`, explicit stdout with `--stdout`, UTF-8 without BOM output, and no rendered body inside JSON.

**Feat F: Agent Diagnostics** — Provide compact/json status, structured errors where feasible, `--check`, `--list-sources`, `--prompt`, and tests covering parser/source/modifier/output behavior.

**What Stays Unchanged**

- Existing commands, aliases, and global flags remain backward compatible.
- `@stdin` / `@file:` / `@env:` remain CLI argument input-source syntax and do not become compose template syntax.
- `--manifest` is not part of this change.
- Generated Temp cleanup is not part of MVP.

### Scope Summary
<!-- MUST end every spec with a File | Change table. -->

| File | Change |
|------|--------|
| `src/cli.rs` | Register `compose` built-in command and route execution. |
| `src/builtins/mod.rs` | Add `compose` module export. |
| `src/builtins/compose/` | New vertical module for CLI args, parser, sources, modifiers, output, and tests. |
| `src/runtime/` | Reuse output/input conventions where compatible; add shared helpers only if they remain generally useful. |
| `tests/compose.rs` | Add integration tests for CLI behavior, stdout/temp output, exec guard, stdin, encoding, modifiers, and diagnostics. |
| `README.md` | Document `compose` command, output contract, template syntax, and examples. |
| `CHANGELOG.md` | Record the new command under unreleased/current version section. |
| `.sspec/changes/26-06-10T01-22_compose/reference/context-composer-prd.md` | Preserve the seed PRD for traceability. |

### Design Reference
<!-- MUST create design.md when the change involves new interfaces, data model changes,
or architectural logic changes. Link here: → See [design.md](./design.md)
Simple changes MAY delete this section and describe the technical approach inline. -->

→ See [design.md](./design.md)
