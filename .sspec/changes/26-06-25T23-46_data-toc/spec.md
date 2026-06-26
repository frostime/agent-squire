---
name: data-toc
status: REVIEW
change-type: single
created: 2026-06-25T23:46:59
reference:
  - source: "reference/gpt-prd.md"
    type: "doc"
    note: "Original product draft for Structured Data TOC."
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

# data-toc

## Problem Statement

Unknown JSON, JSONL, and YAML files currently require agents to read raw content before understanding structure, causing avoidable context waste, weak sampling choices, and higher risk of writing incorrect ad-hoc parsing commands.

Agent Squire already supports structure-first discovery for directories (`file-tree`) and Markdown (`md-toc`). Structured data needs the same discovery step: a bounded table of contents that shows shape, uncertainty, and next useful reads before file contents enter context.

## Proposed Solution

### Approach

Add a built-in `data-toc` command that produces an agent-facing structural table of contents for structured data files. The command starts with a Phase 1 MVP for JSON and JSONL, then keeps YAML support and richer heuristics as explicit later phases within this same change.

`data-toc` is a TOC generator, not a schema generator or query language. It summarizes observed structure within a budget, collapses repeated array indexes, reports observed field presence, exposes uncertainty, and preserves Agent Squire's compact-vs-JSON output pattern.

This is a built-in command rather than an external mapped command because it needs stable CLI integration, stable JSON envelope output, structured aggregation logic, and an agent-facing `--prompt` guide.

### Behavior Contract

**BC-1: CLI surface**
- Surface: `squire data-toc <path>` and alias `squire datatoc <path>`.
- Phase 1 options: `--format auto|json|jsonl`, `--budget small|normal|large`, `--prompt`.
- `--prompt` prints an agent-facing usage guide and exits successfully without requiring a path.
- Unchanged: existing commands, aliases, and global `--print` / `--json` behavior remain backwards compatible.

**BC-2: JSON structure TOC**
- Surface: compact output and JSON envelope output for JSON inputs.
- After: object and array structures are summarized without printing raw values by default; array indexes are normalized to `[]`; observed type and field presence are shown where useful.
- Boundary: Phase 1 MAY use bounded in-process JSON parsing for implementation simplicity if file-size safeguards are enforced; later phases MAY replace or augment this with streaming.
- Error behavior: invalid JSON exits non-zero with a direct parse error.

**BC-3: JSONL record-stream TOC**
- Surface: compact output and JSON envelope output for JSONL / NDJSON inputs.
- After: records are treated as a virtual array of records; heterogeneous shapes are grouped; each displayed group includes a representative `first_line`; output states that groups are approximate.
- Boundary: Phase 1 grouping can be deterministic and simple; it must not introduce ML dependencies.
- Error behavior: invalid JSONL reports the offending 1-based line number.

**BC-4: Budget and uncertainty**
- Surface: all non-prompt `data-toc` outputs.
- After: outputs include budget, completion/sampling status, and notes explaining uncertainty markers such as `?`, `[]`, and approximate JSONL groups.
- Boundary: internal thresholds for sample size, depth, group count, and presence labels are implementation details, not public CLI options.

**BC-5: JSON envelope**
- Surface: `squire --print json data-toc ...` and `squire --json data-toc ...`.
- After: output uses `Envelope<T>` style with `ok`, `command`, `data`, `warnings`, and `meta`; `command` is `data-toc`.
- Boundary: compact output is optimized for agent reading; JSON output is optimized for machine consumption.

**BC-6: Full phased target remains committed**
- Surface: change scope and task plan.
- After: Phase 2 adds YAML via external `yq`; Phase 3 adds dynamic key compression, richer discriminator detection, richer suggested reads, and `--examples` with truncation/redaction.
- Boundary: Phase 1 delivery is acceptable only if later phases remain explicitly represented in the change plan.

### Implementation Changes

**feat(cli): Add data-toc command surface**
- Adds `data-toc` as a built-in command and `datatoc` as its alias. Serves BC-1.

**feat(data-toc): Implement Phase 1 JSON TOC**
- Adds JSON structure aggregation, array index normalization, field presence reporting, compact rendering, and JSON envelope data for JSON inputs. Serves BC-2, BC-4, BC-5.

**feat(data-toc): Implement Phase 1 JSONL TOC**
- Adds bounded line sampling, per-line JSON parsing, structural grouping, representative first-line reporting, and invalid-line diagnostics. Serves BC-3, BC-4, BC-5.

**docs(data-toc): Add agent-facing prompt guide**
- Adds `--prompt` content explaining when to use `data-toc`, commands, output interpretation, and follow-up read patterns. Serves BC-1, BC-4.

**test(data-toc): Cover CLI behavior and output contracts**
- Adds integration and/or module tests for JSON, JSONL, prompt output, alias behavior, JSON envelope shape, and error cases. Serves BC-1 through BC-5.

**plan(data-toc): Preserve later phase commitments**
- Keeps Phase 2 and Phase 3 tasks explicit so MVP delivery does not erase YAML and richer heuristic goals. Serves BC-6.

### Scope Summary

| File | Change | Effort |
|---|---|---|
| `src/cli.rs` | Register `data-toc` command and `datatoc` alias | S |
| `src/builtins/mod.rs` | Export `data_toc` module | XS |
| `src/builtins/data_toc/mod.rs` | New command args, scanning, aggregation, rendering, prompt guide | L |
| `tests/data_toc.rs` | New integration tests for Phase 1 contracts | M |
| `README.md` / `CHANGELOG.md` | Document new command if implementation reaches user-visible completion | S |
| `.sspec/changes/26-06-25T23-46_data-toc/tasks.md` | Track Phase 1/2/3 execution plan | S |

### Design Reference

See [design.md](./design.md).
