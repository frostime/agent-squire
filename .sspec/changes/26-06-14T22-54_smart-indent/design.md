---
name: smart-indent
created: 2026-06-14T22:54:10
updated: 2026-06-15T00:37:36+08:00
---

# smart-indent — Design

## Behavioral Spec

### Matching Chain

```text
exact → loose → smart_indent_probe
```

| Mode | Stage result | Outcome |
|------|--------------|---------|
| any | exact has 1 match | Apply/report existing exact behavior. |
| any | loose has 1 match | Apply/report existing loose behavior. |
| any | exact/loose ambiguous | Preserve existing ambiguity behavior. |
| no `--smart-indent` | smart candidates = 1 | `status: "indent_mismatch"`, `success: false`, include `indent_from`/`indent_to`, suggest `--smart-indent`. |
| no `--smart-indent` | smart candidates > 1 | `status: "search_indent_ambiguous"`, `success: false`, include all related lines. |
| no `--smart-indent` | smart candidates = 0 | Existing fallback result (`already_applied` / `replace_ambiguous` / `search_not_found`). |
| `--smart-indent` | smart candidates = 1 and adjusted REPLACE absent | Apply with `match_mode: "indent_shift"`. |
| `--smart-indent` | smart candidates = 1 and adjusted REPLACE already exists | `status: "already_applied"`. |
| `--smart-indent` | smart candidates > 1 | `status: "search_indent_ambiguous"`; do not apply. |
| `--smart-indent` | REPLACE cannot migrate from `indent_from` | `status: "replace_indent_incompatible"`; do not apply. |

`search_indent_ambiguous` is returned whenever more than one target window can match after base-indent migration, even if all candidates use the same `indent_from -> indent_to`. This prevents broad/fuzzy patch application.

### Base Indent Model

For a block of lines with line endings preserved:

```rust
fn common_base_indent(lines: &[String]) -> Option<String>
```

Rules:

- Non-empty means `strip_line_ending(line).trim_matches([' ', '\t']).is_empty() == false`.
- Only non-empty lines participate.
- The base indent is the longest common prefix consisting of literal spaces/tabs among participating lines.
- Empty block, all-empty block, or no common whitespace prefix returns `Some("")`, not `None`; column-0 is a valid base indent.
- Spaces and tabs are literal bytes/chars; no visual-width conversion.

Examples:

```text
["fn foo() {", "    x", "}"]         → ""
["    fn foo() {", "        x", "    }"] → "    "
["\tkey:", "\t\tchild: v"]          → "\t"
```

### Smart-Indent Candidate Algorithm

```rust
struct IndentShift {
    indent_from: String,
    indent_to: String,
}

struct SmartIndentCandidate {
    start: usize,
    shift: IndentShift,
}

fn find_smart_indent_candidates(region: &[String], search: &[String]) -> Vec<SmartIndentCandidate>
```

Algorithm:

```text
search_from = common_base_indent(search)
normalized_search = strip_base_indent(search, search_from)

for each target window in region with same line count as search:
  target_to = common_base_indent(window)
  normalized_target = strip_base_indent(window, target_to)

  if normalized_search == normalized_target:
    record candidate(start, indent_from=search_from, indent_to=target_to)
```

Important details:

- Comparison is exact after base-indent stripping; smart-indent does not use loose matching internally.
- Exact/loose matching already happened earlier and has higher priority.
- Candidate windows are same line-count windows only; smart-indent never inserts/removes lines.
- A pure indent-only difference is required. Content differences remain `search_not_found`.

### Empty / Whitespace-Only Lines

| Context | Rule |
|---------|------|
| Base indent calculation | Empty/whitespace-only lines are ignored. |
| Matching | Empty/whitespace-only SEARCH lines match empty/whitespace-only target lines after stripping line endings; they do not match non-empty lines. |
| REPLACE migration | Empty/whitespace-only lines are preserved as-is. |

This treats blank lines as logically absent for indent calculation while still preserving block shape and avoiding accidental deletion of non-empty target lines.

### REPLACE Migration

```rust
fn migrate_replace_indent(
    replace_lines: &[String],
    indent_from: &str,
    indent_to: &str,
) -> Result<Vec<String>, ReplaceIndentError>
```

Rules:

- Empty/whitespace-only REPLACE lines are returned unchanged.
- Each non-empty REPLACE line MUST start with `indent_from`.
- Migrated line = `indent_to + line_without_indent_from` with original line ending preserved.
- If any non-empty line does not start with `indent_from`, return `replace_indent_incompatible` and do not write.

Examples:

```text
indent_from=""      indent_to="    "
"fn bar() {"    → "    fn bar() {"
"    x"         → "        x"

indent_from="        "  indent_to="    "
"        key:"      → "    key:"
"          child"   → "      child"
"  bad"            → incompatible
```

### Already-Applied Check

With `--smart-indent`, already-applied must check the migrated REPLACE content:

```text
shift = unique smart-indent candidate from SEARCH
adjusted_replace = migrate_replace_indent(REPLACE, shift.indent_from, shift.indent_to)
find_preferred_matches(region, adjusted_replace)
```

Expected behavior:

```text
first run  → applied / indent_shift
second run → already_applied
```

Without `--smart-indent`, already-applied behavior remains existing exact/loose REPLACE detection; diagnostic probing does not write.

## Interface Contract

### CLI

```bash
asq patch-edit [--smart-indent] ...
asq patch      [--smart-indent] ...   # alias behavior if patch maps to patch-edit
```

- `--smart-indent`: boolean flag, default false.
- No short flag.
- Existing commands/flags remain backward compatible.

### Rust API

Preserve existing public entrypoint:

```rust
pub fn apply_patches(
    patch_text: &str,
    project_root: &Path,
    dry_run: bool,
) -> Vec<PatchApplyResult>
```

Add options-based entrypoint:

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct PatchApplyOptions {
    pub dry_run: bool,
    pub smart_indent: bool,
}

pub fn apply_patches_with_options(
    patch_text: &str,
    project_root: &Path,
    options: PatchApplyOptions,
) -> Vec<PatchApplyResult>
```

Internal helpers may accept `&PatchApplyOptions` or copied booleans.

### Result Metadata

Replace single-delta metadata with directional metadata:

```rust
pub struct PatchApplyResult {
    // existing fields...
    pub indent_from: Option<String>,
    pub indent_to: Option<String>,
}

pub struct PatchMatch {
    // existing fields...
    pub indent_from: Option<String>,
    pub indent_to: Option<String>,
}
```

JSON example:

```json
{
  "success": false,
  "status": "indent_mismatch",
  "match_mode": null,
  "match_line": 12,
  "indent_from": "        ",
  "indent_to": "    ",
  "error": "SEARCH matches after indent migration (8 spaces -> 4 spaces). Use --smart-indent to apply."
}
```

### Status Values

| Status / mode | Type | Meaning |
|---------------|------|---------|
| `indent_mismatch` | failure status | Unique smart-indent candidate found, but `--smart-indent` is off. |
| `search_indent_ambiguous` | failure status | Multiple smart-indent candidates found. |
| `replace_indent_incompatible` | failure status | SEARCH matched, but REPLACE cannot be safely migrated from `indent_from` to `indent_to`. |
| `indent_shift` | match mode | Patch applied/already detected using smart-indent migration. |

## Output Preview

Without `--smart-indent`:

```text
[X] indent_mismatch    # src/main.rs -- SEARCH matches after indent migration (0 spaces -> 4 spaces). Use --smart-indent to apply.
[X] 1 patch(es) failed.
```

With `--smart-indent`:

```text
[OK] applied           # src/main.rs -- Applied (indent_shift @L5, 0 spaces -> 4 spaces)
[OK] All patches succeeded.
```

Ambiguous:

```text
[X] search_indent_ambiguous # config.yml -- SEARCH matches 2 locations after indent migration; narrow the line range.
[X] 1 patch(es) failed.
```

## Test Matrix

| Test | Expected |
|------|----------|
| Existing exact/loose tests | Unchanged. |
| Missing outer indent, flag off | `indent_mismatch`, no write. |
| Missing outer indent, flag on | `applied`, `match_mode=indent_shift`, REPLACE indented to target. |
| SEARCH has too much base indent | `applied` with shorter `indent_to`. |
| Deep YAML base indent shift | Relative child indentation preserved. |
| Empty lines in SEARCH/TARGET | Empty lines ignored for base indent and preserved; non-empty target line cannot match blank SEARCH line. |
| Multiple smart candidates | `search_indent_ambiguous`, no write. |
| Incompatible REPLACE | `replace_indent_incompatible`, no write. |
| Smart-indent idempotency | First run `applied`, second run `already_applied`. |
| Public API compatibility | Old 3-arg `apply_patches` compiles and defaults to strict mode. |
