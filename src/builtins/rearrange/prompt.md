# Squire rearrange format

`asq rearrange` applies an Arrange state-transition DSL. It rewrites only files declared by `arrange` blocks; `share` blocks provide read-only source material.

## Core model

```text
share   = read-only named material
arrange = one target file's complete before -> after transition
```

All ranges are 1-based inclusive logical lines. `A-end` means through EOF and is the recommended way to show full-file coverage. Numeric EOF guards such as `21-200` are accepted when they resolve to the actual final line.

## Syntax

```text
share <slug> = <file>
  <name> = <range>
end share

arrange <file>
  before <file-state>
  after  <file-state>
end arrange

arrange <slug> = <file>
  before <file-state>
  after  <file-state>
end arrange
```

```text
<range>      = N | A-B | A-end
<file-state> = <missing> | <empty> | <sequence>
```

Before sequence items:

```text
<range>
<name> = <range>
<gap:name>
```

After sequence items:

```text
<range>
<name>
<gap:name>
<slug>::<name>
```

Identifiers match `[A-Za-z_][A-Za-z0-9_]*`.

## Invariants

- `before` describes the complete target file pre-state.
- Hidden gaps are invalid. If ranges are not adjacent, declare `<gap:name>` between them.
- `after` can only reference material declared in current `before`, a `share`, or another slugged arrange's named before chunks.
- One normalized path can appear in at most one `arrange` and at most one `share`, and not both.
- There is one pre-state snapshot. Arrange blocks have no execution order.
- `<empty>` is a 0-byte file. `<missing>` means the file does not exist.

## Examples

Single-file reorder:

```text
arrange src/foo.rs
  before api = 1-60, parser = 61-140, rest = 141-end
  after  api, rest, parser
end arrange
```

Explicit gap:

```text
arrange src/foo.rs
  before A = 1-10, <gap:comment>, B = 20-end
  after  B, <gap:comment>, A
end arrange
```

Cross-file extraction:

```text
arrange main = src/foo.rs
  before api = 1-60, parser = 61-140, rest = 141-end
  after  api, rest
end arrange

arrange src/parser.rs
  before <missing>
  after  main::parser
end arrange
```

Create from share:

```text
share tpl = snippets/header.rs
  header = 1-end
end share

arrange src/foo.rs
  before body = 1-end
  after  tpl::header, body
end arrange
```

## CLI

```bash
asq rearrange --stdin < spec.txt        # dry-run preview, no write
asq rearrange --stdin --yes < spec.txt  # apply
asq rearrange -f spec.txt               # read spec file
asq rearrange --prompt                  # print this guide
```

`--dry-run` overrides `--yes`.
