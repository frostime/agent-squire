# Source Resolver

Track knowledge that `src/builtins/source.rs` alone cannot adequately convey.

## Purpose

Unify the per-builtin "expand file/dir/glob inputs into a typed file list plus
unresolved inputs" loop previously duplicated by `toc`, `md-links`,
`md-backlinks`, and `file-info`. Each builtin declares its policy structurally
and injects a `map` closure that converts a resolved `PathBuf` into its own
output item.

## Why a single resolver

The four implementations differed only along a small axis set (gitignore
respect, file filter, dedup strategy, max-files cap, typed output). Before this
module each owned a copy of the glob-magic detection, the directory walker
configuration, the sort, and the missing-input bookkeeping — ~250 lines of
near-duplicate code.

## Policy axes (`SourcePolicy<T>`)

| Axis | Type | Notes |
|---|---|---|
| `root` | `&Path` | Workspace root forwarded to `map` and the `Dedup::ByKey` key. Set to the `CommandContext` cwd by callers. |
| `gitignore` | `GitignoreMode` | `Off` mirrors legacy `walkdir::WalkDir` (no gitignore, no `ALWAYS_SKIP` filter, hidden files included). `Respect` enables .gitignore/git-global/git-exclude and the `ALWAYS_SKIP` filter. |
| `accept_file` | `&dyn Fn(&Path) -> bool` | Higher-order file test applied to dir/glob expansion. Inject `is_markdown_file` for Markdown builtins, `\|_\| true` for `file-info`, or any custom predicate. |
| `filter_explicit_file` | `bool` | When true, an input naming an existing file must pass `accept_file`; rejection becomes unresolved. `md-backlinks` sets this true; `toc`/`md-links`/`file-info` leave it false (explicitly named files are accepted as-is). |
| `filter_glob` | `bool` | When true, glob matches must pass `accept_file`; when false every glob-matched file is accepted (legacy `toc`/`md-links`/`file-info` glob branch only checked `is_file()`). `md-backlinks` sets true. |
| `dedup` | `Dedup` | `None`, `Canonicalize` (file-info), or `ByKey(&dyn Fn(&Path,&Path)->String)` (md-backlinks, keying by display path; last mapping wins and output is key-sorted, matching the prior `BTreeMap`). |
| `max_files` | `Option<usize>` | file-info cap; stop accepting once reached. |
| `map` | `&dyn Fn(PathBuf, &Path) -> Option<T>` | Convert an accepted `PathBuf` to the builtin's output item; `None` drops it. |

## Caller mapping

| Builtin | gitignore | accept_file | filter_explicit_file | dedup | map |
|---|---|---|---|---|---|
| `toc` | `Off` | `ext == "md"` | `false` | `false` | `None` | identity `PathBuf` |
| `md-links` | `Off` | `ext == "md"` | `false` | `false` | `None` | `SourceFile{ path, display_path }` |
| `md-backlinks` | `Respect`/`Off` (via `--no-gitignore`) | `is_markdown_file` | `true` | `true` | `ByKey` (display path) | `SourceFile{ path, display_path }` |
| `file-info` | `Off` | `\|_\| true` | `false` | `false` | `Canonicalize` | `PathBuf` |

`gather` is intentionally **not** routed through this resolver: its inputs are
single-path expansion (`expand_dir`, `expand_glob`, `fzf_*`), not mixed
file/dir/glob lists, and it adds the workspace-root `.gitignore` explicitly
(`add_ignore`) to support non-Git temp workspaces. `gather` shares the
`ALWAYS_SKIP` constant instead.

## Equivalence guarantees

1. **`Off` mode == legacy walkdir traversal**: `ignore::WalkBuilder` is
   configured with `hidden(false)`, all git flags off, no `ALWAYS_SKIP` filter,
   and the final matched set is `sort()`ed lexicographically on `PathBuf` — the
   same post-sort order the original `walkdir` code produced via `found.sort()`.
2. **`Respect` mode == `md-backlinks` walker**: git flags on plus
   `ALWAYS_SKIP` `filter_entry`, matching `discover_corpus`.
3. **Inputs are walked/globbed as given**, relative to the process current
   directory. The CLI has already set the process cwd to the `CommandContext`
   cwd in `cli::try_main`, so relative inputs resolve the same as before; this
   preserves `toc`'s relative-path display (its `base` strip uses the original
   source string, and walker output keeps the input relativity).
4. **Explicit file bypasses gitignore**: an input naming an existing file is
   accepted directly without walking, so `--from <gitignored.md>` still includes
   that file (see `explicit_ignored_file_in_from_is_included`).

## Known behavior deltas after refactor

- `md-backlinks` collapses "explicit non-markdown file rejected" and
  "corpus source not found" into a single unresolved entry. Previously these
  produced distinct warning strings `"non-markdown corpus file skipped: …"` vs
  `"corpus source not found: …"`. The `md-backlinks` call site now emits
  `"corpus source not found: …"` for both. No integration test exercised the
  markdown-skipped branch; the distinction was functional only as a warning
  nuance and is intentionally dropped for the common resolver contract.
- `file-info` keeps its `~`-expansion at the call site (applies to non-glob
  inputs only, matching the original logic that expanded for the file/dir
  existence test but kept raw glob patterns). Unresolved entries for expanded
  inputs now report the expanded string (e.g. `/home/x.txt`) rather than the
  raw `~/x.txt`; the previous code reported the raw source. No test covers this
  edge case. The result is re-sorted lexicographically by canonical path so
  multi-file output order matches the previous BTreeMap ordering.
- `toc`/`md-links` now back the directory walk with `ignore::WalkBuilder`
  configured for `Off` mode (no gitignore, no `ALWAYS_SKIP`, hidden files kept).
  Sorted output is `PathBuf` byte lexicographic via `files.sort()`, matching
  the previous `walkdir::WalkDir` + `found.sort()` ordering (the in-walker
  case-insensitive name sort is overridden by the final lexicographic sort).