---
change: "md-backlinks"
created: 2026-06-03T20:48:45
---

# Design: md-backlinks

## CLI Contract

```rust
#[derive(clap::Args, Debug)]
pub struct MdBacklinksArgs {
    /// Focus pages whose incoming file links should be reported.
    /// Required: at least one page.
    pub pages: Vec<String>,

    /// Corpus roots/files/globs to scan for outgoing links.
    /// Repeatable. Defaults to ["."].
    #[arg(long = "from", value_name = "PATH")]
    pub from: Vec<String>,

    /// Workspace base for workspace-relative file targets.
    #[arg(long, value_name = "DIR")]
    pub workspace: Option<PathBuf>,

    /// Include files normally hidden by .gitignore and built-in skip rules.
    #[arg(long)]
    pub no_gitignore: bool,
}
```

Command semantics:

```text
md-links <sources...>
  source/focus = positional sources
  result       = outgoing links grouped by source file

md-backlinks <pages...> --from <corpus...>
  focus        = positional pages
  corpus       = --from roots/files/globs, default ["."]
  result       = incoming links grouped by focus page
```

## Behavioral Flow

```text
args
  → workspace = args.workspace.unwrap_or(ctx.cwd)
  → focus_set = normalize_focus_pages(args.pages, workspace)
  → corpus_files = discover_backlink_corpus(args.from_or_dot, workspace, policy)
  → for each corpus file S:
       read S
       parse_links(S.content)
       resolve_link(raw, S.path, workspace)
       keep target_type == File
       if resolved_target ∈ focus_set:
           record backlink occurrence under resolved_target
  → output target-grouped result
```

Backlinks are therefore a reverse view of the same resolved forward edges:

```text
S --raw link in Markdown--> resolved file target T

backlinks(X) = all edge occurrences where T == normalize(X)
```

## Corpus Discovery Policy

| Input kind in `--from` | Default behavior | With `--no-gitignore` |
|---|---|---|
| Explicit `.md` / `.markdown` file | Include even if ignored | Include |
| Explicit non-Markdown file | Ignore or warn as non-corpus | Ignore or warn as non-corpus |
| Directory | Recursive Markdown walk via `ignore::WalkBuilder` | Recursive Markdown walk with gitignore + built-in skips disabled |
| Glob | Matched Markdown files included; explicit matches are treated like explicit files | Same |
| Missing input | Warning; if all corpus inputs miss, error | Same |

Default directory walk policy:

```text
hidden(false)
git_ignore(true)
git_global(true)
git_exclude(true)
follow_links(false)
skip names: .git, node_modules, __pycache__, .pytest_cache, .mypy_cache
extensions: md, markdown
```

Rationale: backlinks are only meaningful relative to a visible corpus boundary. The output metadata must expose the corpus and ignore policy.

## Focus Page Normalization

```rust
fn normalize_focus_page(input: &str, workspace: &Path) -> FocusPage {
    // Convert to the same display-path namespace as resolve_link(...).resolved.
    // Preserve missing targets: backlinks to uncreated notes should be representable.
}

struct FocusPage {
    display_path: String,
    exists: bool,
}
```

Rules:

| Input | Normalized display path |
|---|---|
| `notes/foo.md` | `notes/foo.md` |
| `./notes/foo.md` | `notes/foo.md` |
| `/notes/foo.md` | `notes/foo.md` relative to workspace |
| `notes/foo.md#section` | `notes/foo.md` |
| Missing `notes/foo.md` | `notes/foo.md`, `exists=false` |

First version requires focus pages to be filesystem-style paths. Wiki shorthand lookup such as `Foo` → `Foo.md` is not part of the CLI contract unless added explicitly later.

## Data Model

```rust
#[derive(Serialize)]
struct MdBacklinksData {
    pages: Vec<MdBacklinksPage>,
    focus_count: usize,
    corpus_files: usize,
    total_backlinks: usize,
}

#[derive(Serialize)]
struct MdBacklinksPage {
    path: String,
    exists: bool,
    backlinks: Vec<MdBacklink>,
}

#[derive(Serialize)]
struct MdBacklink {
    source: String,
    line_num: usize,
    kind: LinkKind,
    raw: String,
}
```

Envelope:

```json
{
  "ok": true,
  "command": "md-backlinks",
  "data": { "pages": [] },
  "warnings": [],
  "meta": {
    "workspace": ".",
    "from": ["."],
    "respect_gitignore": true,
    "builtin_skip": true,
    "extensions": ["md", "markdown"]
  }
}
```

Compact output shape:

```text
# focus=1 corpus_files=3 backlinks=2 gitignore=true builtin_skip=true
@ notes/foo.md exists=true backlinks=2
README.md:L1|markdown|"notes/foo.md#intro"
docs/index.md:L4|wiki|"notes/foo|Alias"
```

## Shared Core Shape

Preferred minimal sharing:

```rust
pub(crate) struct LinkEdge {
    pub source: String,
    pub line_num: usize,
    pub kind: LinkKind,
    pub raw: String,
    pub target_type: TargetType,
    pub resolved: Option<String>,
    pub exists: Option<bool>,
}

pub(crate) fn analyze_edges(
    source: &SourceFile,
    workspace: &Path,
) -> Result<Vec<LinkEdge>, String>;
```

`md-links` may keep its current file-oriented output code. `md-backlinks` can consume the shared edge builder directly. Any refactor must preserve all existing `tests/md_links.rs` expectations.

## Test-Driven Behavior Matrix

Create `tests/md_backlinks.rs` first and implement until these pass:

| Test | Fixture | Expected |
|---|---|---|
| `json_finds_backlinks_by_resolved_target` | `README.md` links `notes/foo.md#intro`; `docs/index.md` links `../notes/foo.md`; `notes/bar.md` links `[[notes/foo|Foo]]` | Query `notes/foo.md` returns 3 backlinks with source + line + kind + raw |
| `plain_text_filename_is_not_a_backlink` | Corpus file contains plain text `notes/foo.md` and a real link to another target | Query `notes/foo.md` returns 0 backlinks |
| `default_corpus_respects_gitignore` | `.gitignore` ignores `ignored.md`; `ignored.md` links focus; visible file does not | Default returns 0; `--no-gitignore` returns 1 |
| `explicit_ignored_file_in_from_is_included` | `.gitignore` ignores `ignored.md`; command uses `--from ignored.md` | Returns backlink despite default ignore policy |
| `compact_output_is_dense_and_grouped_by_page` | One focus page, one backlink | stdout contains summary, `@ target`, and `source:Lline|kind|raw` |
| `missing_focus_page_can_have_backlinks` | Corpus links `[ghost](missing.md)` while `missing.md` absent | Query `missing.md` returns `exists=false` with backlink |
| `md_links_existing_behavior_remains_unchanged` | Existing `tests/md_links.rs` | All current tests still pass |

Unit-level tests may be added for focus normalization and corpus discovery only when integration tests leave edge cases ambiguous.

## Non-Goals

- No `rg` shell-out or external binary dependency.
- No full workspace reverse index mode without focus pages in the first version.
- No URL or SiYuan block backlinks in the first version; only `target_type=file` participates.
- No fuzzy title matching or wiki basename search beyond current `resolve_link` behavior.
