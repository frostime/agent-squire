---
change: "data-toc"
created: 2026-06-25T23:46:59
---

# Design: data-toc

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

## Phase Map

| Phase | Status Target | User-visible capability |
|---|---|---|
| Phase 1 | MVP implementation | JSON + JSONL structural TOC, `--budget`, `--prompt`, compact output, JSON envelope |
| Phase 2 | Planned in same change | YAML support via external `yq`; YAML output marks `parsed_as=json` and approximation |
| Phase 3 | Planned in same change | Dynamic key compression, smarter JSONL discriminator labels, richer suggested reads, `--examples` with truncation/redaction |

Phase 1 must not close the change as complete unless Phase 2/3 are either implemented or explicitly deferred through SSPEC review.

## CLI Interface

```rust
#[derive(clap::Args, Debug)]
pub struct DataTocArgs {
    /// JSON, JSONL, or YAML file path. Not required with --prompt.
    pub path: Option<std::path::PathBuf>,

    #[arg(long, value_enum, default_value_t = DataFormat::Auto)]
    pub format: DataFormat,

    #[arg(long, value_enum, default_value_t = Budget::Normal)]
    pub budget: Budget,

    #[arg(long, help = "Print the agent-facing data-toc guide")]
    pub prompt: bool,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DataFormat { Auto, Json, Jsonl }

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Budget { Small, Normal, Large }
```

CLI registration:

```rust
#[command(
    name = "data-toc",
    alias = "datatoc",
    about = "Show JSON/JSONL structure before reading data"
)]
DataToc(builtins::data_toc::DataTocArgs),
```

## Runtime Flow

```text
run(args, ctx)
├─ if args.prompt → print DATA_TOC_PROMPT → exit 0
├─ require path
├─ detect/validate format
├─ resolve budget profile
├─ analyze input
│  ├─ json  → analyze_json(path, budget)
│  └─ jsonl → analyze_jsonl(path, budget)
└─ render
   ├─ PrintMode::Json → Envelope<DataTocData>
   └─ _               → compact text
```

## Data Model

```rust
#[derive(serde::Serialize)]
struct DataTocData {
    path: String,
    format: DataFormat,
    mode: TocMode,
    complete: bool,
    root: TocNode,
    summary: DataSummary,
    notes: Vec<String>,
    suggested_reads: Vec<String>,
    record_groups: Vec<RecordGroup>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum TocMode { StructureToc, RecordStreamToc }

#[derive(serde::Serialize)]
struct TocNode {
    name: String,
    path: String,
    kind: NodeKind,
    presence: Option<Presence>,
    observed_items: Option<usize>,
    shape_count: Option<usize>,
    children: Vec<TocNode>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum NodeKind {
    Null,
    Boolean,
    Number,
    String,
    Object,
    Array,
    Mixed,
}

#[derive(serde::Serialize)]
struct Presence {
    observed: usize,
    total: usize,
}

#[derive(serde::Serialize)]
struct RecordGroup {
    label: String,
    rows: usize,
    first_line: usize,
    shape: Vec<String>,
}
```

Notes:
- `record_groups` is empty for JSON.
- `suggested_reads` can be conservative in Phase 1; it must not expose raw values.
- `TocNode.path` uses normalized paths such as `$.runs[].metrics.acc`.

## JSON Analysis

Phase 1 favors a contained implementation over premature streaming complexity.

```text
serde_json::from_reader(file)
→ walk value with budget depth/child/item limits
→ normalize array indexes to []
→ aggregate child nodes and observed presence inside sampled arrays
→ render tree
```

Budget profiles are internal constants:

| Budget | Intended use | Example internal limits |
|---|---|---|
| `small` | quick structure preview | shallow depth, fewer array items, fewer children |
| `normal` | default agent use | moderate depth and samples |
| `large` | complex files | deeper/larger samples within safe caps |

JSON array handling:

```text
$.runs[0].metrics.acc
$.runs[1].metrics.acc
        ↓
$.runs[].metrics.acc
```

Presence rule:

```text
field present in all observed objects   → `name string 64/64` or omit count when obvious
field present in subset of objects      → `name string? 11/64`
field observed with multiple value kinds → `name mixed 64/64`
```

## JSONL Analysis

```text
read lines up to budget.max_jsonl_lines
→ parse each non-empty line as serde_json::Value
→ extract bounded structural signature
→ group by exact signature
→ merge/render top groups within budget.max_groups
→ compute aggregate virtual root fields and group first_line
```

Structural signature example:

```text
$.type:string
$.timestamp:string
$.error:object
$.error.code:number
$.error.message:string
```

Phase 1 group labels:

| Condition | Label |
|---|---|
| A top-level discriminator field clearly maps to a group | `type=error`, `kind=message`, etc. |
| No stable discriminator is found | `shape#1`, `shape#2`, ... |
| Minor shapes exceed output budget | `other` |

Candidate discriminator fields, checked after shape grouping:

```text
type, kind, event_type, event, action, op, role, level, category
```

## Compact Output Preview

JSON:

```text
# data-toc result.json
format=json mode=structure-toc complete=false budget=normal

$ object
└─ runs array<object> observed_items≈64 shape≈2
   └─ [] object
      ├─ id string 64/64
      ├─ metrics object 64/64
      │  └─ acc number 64/64
      └─ notes string? 11/64

Notes:
- Output is based on bounded structural scanning.
- `?` means not present in all observed samples.
- Array indexes are collapsed into [].

Suggested reads:
- jq '.runs[0:5]' result.json
```

JSONL:

```text
# data-toc logs.jsonl
format=jsonl mode=record-stream-toc complete=false budget=normal sampled_lines=1000

$ array<record> virtual=jsonl groups≈3
└─ [] object
   ├─ type string 1000/1000
   ├─ payload object? 716/1000
   └─ error object? 91/1000

Record groups:
- type=message rows=521 first_line=1
  shape: object{type,timestamp,user,payload}
- type=error rows=91 first_line=37
  shape: object{type,timestamp,error}

Notes:
- JSONL records appear heterogeneous.
- Groups are approximate structural clusters.
```

## JSON Envelope Preview

```json
{
  "ok": true,
  "command": "data-toc",
  "data": {
    "path": "logs.jsonl",
    "format": "jsonl",
    "mode": "record_stream_toc",
    "complete": false,
    "root": { "name": "$", "path": "$", "kind": "array", "children": [] },
    "summary": {},
    "notes": ["Groups are approximate structural clusters."],
    "suggested_reads": ["sed -n '37p' logs.jsonl | jq ."],
    "record_groups": []
  },
  "warnings": [],
  "meta": {
    "budget": "normal",
    "schema_version": 1
  }
}
```

## Prompt Guide Content Outline

`data-toc --prompt` prints a static guide with this structure:

```text
# Squire data-toc guide

## When to use
- Unknown JSON/JSONL/YAML structure before reading raw content.

## Commands
asq data-toc result.json
asq data-toc logs.jsonl --format jsonl
asq --print json data-toc result.json

## Output interpretation
- `[]` means array indexes are collapsed.
- `?` means observed in only part of the sample.
- `complete=false` means bounded or sampled scan.
- JSONL groups are approximate structural clusters.

## Follow-up reads
- Use `jq` for JSON slices.
- Use `sed -n '<line>p' file.jsonl | jq .` for representative JSONL rows.
```

## Error Contract

| Case | Behavior |
|---|---|
| `--prompt` without path | Print guide, exit 0 |
| Missing path without `--prompt` | Exit non-zero with direct message |
| Unsupported/undetected format | Exit non-zero with direct message |
| Invalid JSON | Exit non-zero with parse context |
| Invalid JSONL | Exit non-zero with 1-based line number |
| YAML in Phase 1 | Exit non-zero with message that YAML support is planned, unless Phase 2 has been implemented |

## Later Phase Design Hooks

| Later capability | Phase 1 design hook |
|---|---|
| YAML via `yq` | Keep `DataFormat` extensible; add `Yaml` only in Phase 2 |
| Dynamic key compression | Keep node construction centralized so sibling key heuristics can transform object children |
| `--examples` | Keep value rendering absent by default; add explicit value sampler and redactor later |
| Smarter suggested reads | Keep `suggested_reads` as structured list now, improve generation later |
| Streaming JSON | Keep analysis behind `analyze_json` boundary so implementation can move from DOM to streaming without CLI change |
