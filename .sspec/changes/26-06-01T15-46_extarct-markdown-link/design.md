---
change: "extarct-markdown-link"
created: 2026-06-01T15:46:16
---

# Design: extarct-markdown-link

## UX Goal

`md-links` helps agents build a Markdown reference graph quickly:

```text
Markdown files
  → extract link-like references
  → classify URL / file / SiYuan block / unknown
  → resolve file refs against source file + workspace
  → report graph-ready occurrences with existence checks
```

Primary consumer: agent/tooling that wants reliable `source → target` edges, not a human Markdown renderer.

## CLI Contract

```text
squire md-links [SOURCES]... [--workspace DIR]
asq md-links README.md docs --workspace .
asq --print json md-links .sspec
```

| Input | Behavior |
|---|---|
| no source | scan `.` |
| file source | scan that file |
| directory source | recursively scan `.md` files |
| glob source | scan matched files |
| `--workspace DIR` | base directory for workspace-relative refs |
| no `--workspace` | effective `cwd` after global `--cwd` |

## Output Contract

Each extracted reference is an occurrence, not a deduplicated edge.

```json
{
  "line_num": 12,
  "kind": "markdown",
  "raw": "docs/intro.md#install",
  "target_type": "file",
  "resolved": "docs/intro.md",
  "exists": true
}
```

Fields:

| Field | Meaning |
|---|---|
| `path` | source Markdown file path, workspace-relative when possible |
| `line_num` | 1-based source line |
| `kind` | syntax that produced the reference |
| `raw` | captured target, trimmed, preserving query/fragment/title text when useful |
| `target_type` | `url` / `file` / `siyuan_block` / `unknown` |
| `resolved` | normalized target for file refs or URL string for URL refs |
| `exists` | `true/false` only for file refs; `null` otherwise |

JSON envelope:

```json
{
  "ok": true,
  "command": "md-links",
  "data": {
    "files": [],
    "count": 0,
    "total_links": 0,
    "total_file_links": 0,
    "total_existing_file_links": 0
  },
  "warnings": [],
  "meta": { "workspace": "..." }
}
```

Compact output groups by source file to avoid repeating long file names. Inside each group, lines use dense fields rather than padded columns. Robust machine consumption should still use `--print json`.

```text
# files=3 links=14 file=9 exists=7
@ README.md
L12|markdown|file|ok|"docs/intro.md#install"|"docs/intro.md"
L18|image|file|missing|"./assets/logo.png"|"assets/logo.png"
L25|wiki|file|ok|"[[Design]]"|"Design.md"
L31|markdown|url|-|"https://example.com"
L40|siyuan_block|siyuan_block|-|"20260531010806-35bkoxa '2026-05-31'"|"20260531010806-35bkoxa"
```

Compact grammar:

```text
# files=<n> links=<n> file=<n> exists=<n>
@ <source-file>
L<line>|<kind>|<target_type>|<status>|<raw-json-string>[|<resolved-json-string>]
```

| Part | Meaning |
|---|---|
| `@ <source-file>` | current source file; shown once per file group |
| `L<line>` | 1-based line in current source file |
| `status` | `ok` / `missing` for file refs, `-` otherwise |
| string fields | JSON-escaped strings, so `|`, quotes, tabs, and newlines remain parseable |
| summary line | starts with `#`; consumers MAY ignore it |
```

## Syntax Coverage

| Syntax | Example | Kind | Target handling |
|---|---|---|---|
| Markdown link | `[intro](docs/intro.md)` | `markdown` | URL/file/unknown |
| Markdown image | `![logo](./logo.png)` | `image` | URL/file/unknown |
| Wiki link | `[[Design]]`, `[[docs/intro#A]]` | `wiki` | workspace file ref; `.md` fallback |
| Inline code path | `` `src/lib.rs` `` | `code_span` | URL/file/unknown if it looks path-like |
| Angle ref | `<src/lib.rs>`, `<https://x>` | `angle` | URL/file/unknown if it looks path-like |
| SiYuan block ref | `((20260531010806-35bkoxa '2026-05-31'))` | `siyuan_block` | parse ID + optional title |

Skipped:

| Case | Behavior |
|---|---|
| fenced code block | ignored |
| empty target | ignored |
| non-link inline text | ignored |
| unsupported schemes | `unknown`, except accepted URL schemes below |

Accepted URL schemes:

```text
http://
https://
siyuan://
```

## Path Classification

A captured target becomes `file` when it matches any rule:

| Rule | Examples |
|---|---|
| explicit relative path | `./a.md`, `../a.md` |
| absolute-looking path | `/src/a.md`, `/home/me/a.md`, `C:/x/a.md`, `C:\x\a.md` |
| contains path separator | `docs/a.md`, `docs\a.md` |
| known file extension | `README.md`, `logo.png`, `main.rs` |
| wiki target | `[[Foo]]`, `[[dir/Foo]]` |
| code/angle target that looks path-like | `` `src/main.rs` ``, `<docs/a.md>` |

Otherwise:

| Target | Type |
|---|---|
| `http://...`, `https://...`, `siyuan://...` | `url` |
| valid SiYuan block pattern inside `((...))` | `siyuan_block` |
| other content | `unknown` or ignored when syntax requires path-like content |

## File Resolution Algorithm

Resolution normalizes `\` to `/` for display and existence checks where platform-safe.

```text
resolve_file_ref(raw, source_file, workspace):
  target = strip_fragment_query(raw)
  target = normalize_separators(target)  # \ → /

  if target is Windows absolute:
      return target

  if target starts with /:
      first try workspace + target_without_leading_slash
      if missing and target exists as OS absolute:
          return OS absolute target
      else:
          return workspace + target_without_leading_slash

  if target starts with ./ or ../:
      return source_file.parent + target

  return workspace + target
```

Wiki fallback:

```text
resolve_wiki(raw):
  target = strip_alias_fragment_query(raw)
  candidate = resolve_file_ref(target, source_file, workspace)
  if candidate exists:
      return candidate
  if target has no extension:
      try candidate + ".md"
  return candidate or candidate.md fallback path
```

Fragment/query behavior:

| Raw | Existence check target | `raw` preserved? |
|---|---|---|
| `docs/a.md#h1` | `docs/a.md` | yes |
| `docs/a.md?x=1` | `docs/a.md` | yes |
| `[[docs/a#h1]]` | `docs/a.md` fallback | yes |

## SiYuan Block Ref Handling

Recognized shape:

```text
((<block-id> '<title>'))
((<block-id> "<title>"))
```

Where block id is:

```text
14 digits + "-" + fixed-length lowercase/digit suffix
example: 20260531010806-35bkoxa
```

Output concept:

```json
{
  "kind": "siyuan_block",
  "raw": "20260531010806-35bkoxa '2026-05-31'",
  "target_type": "siyuan_block",
  "resolved": "20260531010806-35bkoxa",
  "exists": null
}
```

SiYuan `siyuan://...` links are treated as URL refs.

## Graph Interpretation

Consumers can derive edges as:

```text
for each file in data.files:
  source = file.path
  for each link in file.links:
    if link.target_type == "file" and link.exists == true:
      edge(source, link.resolved)
    if link.target_type == "url":
      external_edge(source, link.resolved)
    if link.target_type == "siyuan_block":
      siyuan_edge(source, link.resolved)
```

No deduplication is performed by the CLI; occurrence-level output preserves line-level evidence.

## Implementation Shape

Estimated complexity is high enough to split the builtin into focused files instead of a single large `mod.rs`. This follows the existing `patch_edit` precedent for non-trivial builtins while keeping one vertical command directory.

```text
src/builtins/md_links/
  mod.rs       # CLI args + run orchestration
  model.rs     # serializable output enums/structs
  sources.rs   # file/dir/glob source expansion
  parse.rs     # fenced-block aware reference scanners
  resolve.rs   # target classification + path/wiki/SiYuan resolution
  output.rs    # compact + JSON output formatting
```

Why split:

| Concern | Expected branching |
|---|---|
| syntax scanning | markdown/image/wiki/code/angle/SiYuan + fenced-code skipping |
| path resolution | workspace vs source-relative vs OS absolute fallback + slash normalization |
| output | compact line protocol + JSON envelope |
| tests | easier to keep behavior tests at CLI level while unit-testing only complex pure parsing/resolution if needed |

## Test Strategy

Follow `write-good-tests`: prefer observable CLI behavior through `assert_cmd`; unit-test pure parsing/resolution only where integration setup would obscure edge cases.

Behavior tests SHOULD cover:

| Behavior | Public assertion |
|---|---|
| extracts representative syntaxes | `asq --print json md-links file.md` contains expected occurrences |
| resolves source-relative and workspace-relative paths | `resolved` + `exists` fields match tempdir fixtures |
| `/src` fallback rule | workspace-relative path wins before OS-root fallback |
| normalizes separators | backslash input resolves/displays with `/` |
| skips fenced code blocks | fenced fake links are absent |
| handles SiYuan refs | `siyuan_block` ID/title and `siyuan://` URL are classified correctly |
| compact output is dense and parseable | no aligned padding; fields follow line grammar |

Avoid:

- snapshotting the full JSON envelope;
- one test per internal function;
- tests coupled to file split or helper names.

## Compatibility

- Existing commands and aliases remain unchanged.
- The new command is additive: `md-links` alias `mdlinks`.
- `--cwd` still changes process cwd before execution.
- `--workspace` only affects reference resolution.
