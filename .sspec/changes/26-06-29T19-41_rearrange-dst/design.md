---
change: "rearrange-dst"
created: 2026-06-29T19:41:26
---

# Design: rearrange-dst

## Design pressures

| Pressure | Architectural consequence |
|---|---|
| Whole-file `before -> after` | Core model is file state transition, not action list |
| Pre-state snapshot | All reads/validations happen before any write; no arrange execution order |
| Provenance audit | One material registry owns all item/reference binding |
| Path identity invariant | Path resolver is part of DSL semantics, not generic I/O helper |
| Multi-file writes | Planner produces complete outcomes; applier writes only after successful validation |
| Reviewable preview | Output renders structured plan facts; output layer does not reinterpret DSL |

## Module structure

```text
src/builtins/rearrange/
  mod.rs        CLI args / input source / run orchestration
  error.rs      ErrorCode + RearrangeError + source context
  ast.rs        parsed DSL AST and plan-facing vocabulary
  parser.rs     line-role tokenizer + block/payload parser
  path.rs       PathResolver / PathKey / display path
  textio.rs     decode/read/render/write/delete/mkdir
  plan.rs       snapshot + validation + material registry + materialization
  output.rs     compact/json preview rendering
  prompt.md     agent-facing DST DSL guide
```

Dependency direction:

```text
mod.rs
  ├─ parser -> ast,error
  ├─ plan   -> ast,error,path,textio
  └─ output -> plan,error
```

`output.rs` depends on plan facts only. It must not parse DSL strings or recompute provenance.

## Runtime flow

```text
read spec input
  ↓
parser::parse(input) -> SpecAst
  - syntax only
  - source line retained
  - RangeExpr.raw retained
  ↓
plan::execute(spec, cwd, write)
  ↓
resolve_paths(ast, cwd) -> ResolvedSpec
  - canonical identity
  - duplicate path/slug validation
  - prefix conflict validation
  ↓
read_snapshot(resolved) -> Snapshot
  - read each existing share/arrange path once
  - record missing targets
  ↓
validate_and_bind(snapshot) -> Plan
  - before state validation
  - coverage/gap validation
  - share validation
  - material registry construction
  - after reference validation
  ↓
materialize(plan) -> Outcome
  - target desired states
  - preview summaries
  ↓
output::render(outcome)
  ↓
if write: apply(outcome)
  - create_dir_all(parent)
  - write/delete files
```

## Core types

```rust
struct SpecAst {
    shares: Vec<ShareAst>,
    arranges: Vec<ArrangeAst>,
}

struct ShareAst {
    slug: Ident,
    path: RawPath,
    items: Vec<ShareItemAst>,
}

struct ArrangeAst {
    slug: Option<Ident>,
    path: RawPath,
    before: FileState<BeforeItem>,
    after: FileState<AfterItem>,
}

enum FileState<T> {
    Missing,
    Empty,
    Sequence(Vec<T>),
}

enum BeforeItem {
    Anonymous(RangeExpr),
    Named { name: Ident, range: RangeExpr },
    Gap { name: Ident },
}

enum AfterItem {
    Anonymous(RangeExpr),
    Local { name: Ident },
    Gap { name: Ident },
    External { slug: Ident, name: Ident },
}

struct RangeExpr {
    raw: String,        // after anonymous range uses exact literal match
    start: usize,
    end: RangeEnd,      // Line(n) | End
}
```

`RangeExpr.raw` is semantic, not cosmetic: `after 1-20` may only reference an anonymous `before 1-20`, not a named range with the same resolved coordinates.

## Parser strategy

Use patch-edit's useful idea (line roles before payload parsing) without using regex block extraction as the main parser.

```text
source lines
  -> LineToken { line_no, role, raw }
  -> block parser
  -> payload parser
  -> SpecAst
```

Line roles:

```text
Blank | Comment
ShareOpen(slug,path)
ArrangeOpen(slug?,path)
ShareItem(name,range)
Before(raw_state)
After(raw_state)
EndShare
EndArrange
Invalid
```

State machine:

```text
Top
  share open     -> InShare
  arrange open   -> InArrange(expect before)

InShare
  share item*    -> InShare
  end share      -> Top

InArrange
  before         -> expect after
  after          -> expect end arrange
  end arrange    -> Top
```

Reason not to use patch-edit's regex extractor: rearrange documents must be wholly valid DSL; no surrounding prose is ignored, and every line inside a block has semantic meaning.

## Path identity

```rust
struct ResolvedPath {
    display: String,
    abs: PathBuf,
    key: PathKey,
}
```

Resolution policy:

| Case | Behavior |
|---|---|
| Existing file | `canonicalize` full path; key from canonical path |
| Missing arrange target | canonicalize nearest existing ancestor + normalized suffix |
| Missing parent dirs | allowed; apply creates parent directories |
| Existing ancestor is file | fail |
| Path escapes `cwd` after symlink/canonicalization | fail |
| Duplicate key | fail |
| share and arrange same key | fail |
| target path prefix conflict | fail |

Prefix conflict example that fails:

```text
arrange foo
  before <missing>
  after  <empty>
end arrange

arrange foo/bar.rs
  before <missing>
  after  other::chunk
end arrange
```

## Material registry

```rust
struct MaterialRegistry {
    local: HashMap<ArrangeId, LocalMaterials>,
    exports: HashMap<(Slug, Name), MaterialId>,
    materials: Vec<Material>,
}

struct Material {
    id: MaterialId,
    origin: MaterialOrigin,
    lines: Vec<String>,
    exportable: bool,
}
```

Owners:

| Knowledge | Owner |
|---|---|
| `share slug::name` binding | material registry |
| `arrange slug::name` export | material registry |
| anonymous range exact raw match | local materials |
| gap actual range and lines | local materials |
| after reference validation | material registry |
| preview provenance | plan summary derived from registry |

## Validation order

```text
1. Names and path identities
2. Read pre-state snapshot
3. Validate shares
4. Validate each arrange before state
5. Build local materials + exports
6. Validate each arrange after state
7. Materialize desired target states
8. Apply writes only if requested
```

Important invariants:

| Invariant | Validation owner |
|---|---|
| before covers whole existing file | `validate_before_sequence` |
| hidden gap rejected unless explicit | `validate_before_sequence` |
| empty gap rejected | `validate_before_sequence` |
| named range cannot be referenced as bare range | after resolver via anonymous map |
| arrange exports only named before chunks | export builder |
| gap never exported | export builder |
| `<missing> -> <missing>` invalid | state transition validation |

## Output contract

Compact output renders structure, not a default whole-file diff:

```text
rearrange 2 files (dry-run)

share tpl = snippets/header.rs
  header = 1-end -> 12 lines

target main = src/foo.rs
  before: api=1-60, parser=61-140, rest=141-end
  after : api, rest
  exports: main::api, main::parser, main::rest
  effects: rewrite file

target src/parser.rs
  before: <missing>
  after : main::parser
  effects: create file (80 lines)

No file written. Pass --yes to apply.
```

JSON keeps a structured form of the same facts under the existing envelope.

## YAGNI cuts

- No generic parser framework shared with patch-edit.
- No old v1 DSL compatibility mode.
- No AST/LSP/symbol range discovery.
- No rollback journal.
- No default full-file diff rendering.
- No alternate DSL spellings or aliases.
