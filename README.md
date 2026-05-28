# Agent Squire

`agent-squire` is a local CLI toolbox for humans and agents.

The primary binary is:

```bash
squire
```

Two aliases are also installed:

```bash
agent-squire
asq
```

## Design constraints

- Flat built-in commands first. No premature `fs tree`, `md toc`, or `web fetch` namespace split.
- No thin wrappers for trivial shell commands.
- Built-ins live under `src/builtins/` as vertical command modules.
- External mapping is intentionally small: map a command name to a raw script/CLI invocation.
- CLI parsing is handled by `clap`, not hand-written argument parsing.
- Agent-facing help and structured output are first-class.

## Built-ins

```bash
squire tree .
squire info README.md
squire toc README.md
squire patch-edit @file:fix.patch --dry-run --print json
```

Aliases retained for migration:

```bash
squire view-tree .
squire fileinfo README.md
squire mdtoc README.md
squire patch @file:fix.patch --dry-run
```

## Global options

```bash
squire --cwd /path/to/project tree .
squire --print json info README.md
squire info README.md --print json
```

`--print` is a global option and may appear before or after subcommands.

Supported modes:

- `compact`: default human-readable output
- `json`: machine-readable JSON envelope
- `ndjson`: reserved for streaming commands
- `text`: plain body text where applicable
- `raw`: external-command passthrough mode

## Input sources

Commands that accept text input support:

```text
@stdin        read from stdin
@file:path    read from a file
@env:NAME     read from an environment variable
@@file:path   pass literal "@file:path"
```

Examples:

```bash
cat fix.patch | squire patch-edit @stdin --dry-run
squire patch-edit @file:fix.patch --dry-run --print json
```

## External command mappings

Global config:

```text
~/.config/agent-squire/config.toml
```

Project config:

```text
.agent-squire.toml
```

Example:

```toml
[commands.fetch]
run = ["python3", "~/skills/fetch_web.py"]
summary = "Fetch readable webpage content."
print_aware = true
expand_args = false
```

Then:

```bash
squire fetch https://example.com --print text
```

If `print_aware = true`, Squire appends `--print <mode>` to the mapped command whenever the global print mode is not `compact`.

## Patch-edit compatibility

`patch-edit` ports the original Python SEARCH/REPLACE patch algorithm:

- LRR-style patch block extraction
- exact match first, loose match second
- optional 1-based line ranges
- `CREATE`, `OVERWRITE`, and targeted `SEARCH`
- already-applied detection
- ambiguous search / replace detection
- same-file multi-patch two-phase matching
- overlap detection before writing
- atomic writes with best-effort permission preservation
- newline-style preservation
- UTF-8 / UTF-8 BOM / GBK / Windows-1252 decoding fallback

Non-dry-run writes require `--yes`:

```bash
squire patch-edit @file:fix.patch --yes
```

Validate safely first:

```bash
squire patch-edit @file:fix.patch --dry-run --print json
```

Interactive input opens `$EDITOR`/`$VISUAL` when configured, dry-runs first, optionally shows a unified diff, then asks before applying:

```bash
asq patch -i
```

Without `$EDITOR`, paste into the terminal and submit with a single `.` line.

## Development

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt
```

This archive includes unit/integration tests that lock the initial behavior. The environment used to generate this zip did not include a Rust toolchain, so the tests are present but were not executed here.
