# Source Resolver

Track knowledge that `src/shared/file_sources.rs` alone cannot adequately convey.

## Purpose

Unify the per-builtin "expand file/dir/glob inputs into a typed file list plus
unresolved inputs" loop previously duplicated by `toc`, `md-links`,
`md-backlinks`, and `file-info`. Each builtin declares its policy structurally
and injects a `map` closure that converts a resolved `PathBuf` into its own
output item.

## Why a single resolver

The four implementations differed along a bounded set of axes: directory
selection, glob-directory expansion, file filtering, deduplication, maximum
files, and typed output. The resolver owns expansion, ordering, limits, and
unresolved-source accounting while callers declare those differences.

## Policy axes (`SourcePolicy<T>`)

| Axis | Type | Notes |
|---|---|---|
| `root` | `&Path` | Workspace root forwarded to `map` and the `Dedup::ByKey` key. Set to the `CommandContext` cwd by callers. |
| `directory_selection` | `DirectorySelection` | `All` uses plain `WalkDir` and applies no ignore rules. `Filtered(IgnoreSources)` uses `WalkBuilder` and applies the selected rule sources. |
| `glob_directory_mode` | `GlobDirectoryMode` | `Skip` ignores directories returned by glob matching. `Recurse` expands them through the same directory path used by explicit directory inputs. |
| `accept_file` | `&dyn Fn(&Path) -> bool` | File predicate applied during directory recursion and, when enabled, glob expansion. |
| `filter_explicit_file` | `bool` | When true, an explicit existing file must pass `accept_file`; rejection becomes unresolved. |
| `filter_glob` | `bool` | When true, glob-matched files must pass `accept_file`; recursive glob directories always use the normal directory predicate. |
| `dedup` | `Dedup` | `None`, `Canonicalize`, or caller-provided `ByKey`. |
| `max_files` | `Option<usize>` | Stop accepting files once the mapped, deduplicated result reaches the cap. |
| `map` | `&dyn Fn(PathBuf, &Path) -> Option<T>` | Convert an accepted file path to the caller's output item. |

## Ignore sources

`IgnoreSources` is a bitflag set. Each flag represents one independent source
of file exclusion:

| Flag | Rules |
|---|---|
| `DOT_IGNORE` | `.ignore` files |
| `GIT` | `.gitignore`, global Git ignore, and Git exclude |
| `BUILTIN_DIRS` | Directory names in `ALWAYS_SKIP` |

These flags do not control hidden files, glob recursion, predicates, or
traversal depth. Filtered walks set `hidden(false)` so dotfiles remain visible
unless one of the selected rule sources excludes them.

## Caller mapping

| Builtin | Directory selection | Glob directories | Explicit file filter | Glob file filter | Dedup |
|---|---|---|---|---|---|
| `toc` | `All` | `Skip` | off | off | `None` |
| `md-links` | `All` | `Skip` | off | off | `None` |
| `md-backlinks` default | `DOT_IGNORE \| GIT \| BUILTIN_DIRS` | `Skip` | markdown | markdown | `ByKey` |
| `md-backlinks --no-gitignore` | `DOT_IGNORE \| BUILTIN_DIRS` | `Skip` | markdown | markdown | `ByKey` |
| `file-info` | `All` | `Recurse` | off | off | `Canonicalize` |

`gather` is intentionally not routed through this resolver: its inputs are
single-path expansion (`expand_dir`, `expand_glob`, `fzf_*`), not mixed
file/dir/glob lists. `gather` shares `ALWAYS_SKIP` only.

## Equivalence guarantees

1. `DirectorySelection::All` uses `walkdir::WalkDir`, preserving the original
   `toc`, `md-links`, and `file-info` behavior: `.ignore`, Git ignore rules,
   built-in skip names, and hidden-file filtering are not applied.
2. Directories matched by `file-info` glob patterns are recursively expanded,
   including the legacy behavior that an existing empty directory resolves the
   glob even when it contributes no files.
3. Filtered traversal applies exactly the `IgnoreSources` bits supplied by the
   caller. `--no-gitignore` removes only `GIT`; `.ignore` and built-in skip
   directories remain active.
4. Inputs are resolved relative to the process current directory. The CLI sets
   it to `CommandContext.cwd` before invoking builtins.
5. Explicit files bypass directory ignore rules. An explicitly named ignored
   Markdown file remains eligible for `md-backlinks --from`.
6. Result sorting, deduplication, `max_files`, and mapping occur after the same
   acceptance boundaries as before unless stated above.

## Known behavior deltas after refactor

- `md-backlinks` consolidates explicit non-Markdown rejection and missing corpus
  sources into the warning `"corpus source not found: ..."`.
- `file-info` unresolved entries for `~`-expanded inputs report the expanded
  path instead of the original `~/...` source.
- `md-backlinks --no-gitignore` now follows its literal name: it disables Git
  ignore rules only. Built-in skip directories remain active, and JSON/compact
  metadata report `builtin_skip=true`.
