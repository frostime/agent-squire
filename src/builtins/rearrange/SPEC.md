# `asq rearrange` Behaviour and Maintenance Spec

This file is the developer-facing contract for `src/builtins/rearrange/`.
It describes the behaviour that parser, planner, preview, apply, and tests must preserve.
It is not a CLI help page; the short user-facing guide lives in `prompt.md`.

When implementation behaviour changes intentionally, update this file and the matching tests in the same change.

## 1. Core model

`asq rearrange` is a line-range based file-state transition tool.

The DSL declares:

```text
share   = read-only named material from a pre-state file
arrange = one target file's complete before -> after transition
```

It is declarative. It does not execute a sequence of edit actions. All material comes from files as they existed before the invocation starts.

Non-goals:

- Discover symbols, functions, modules, imports, or AST structure.
- Infer omitted ranges.
- Synthesize arbitrary literal text in `after`.
- Repair imports, module declarations, formatting, or compilation errors.
- Depend on block execution order.

## 2. Line-oriented syntax

The parser is line-oriented.

- Leading/trailing whitespace on each line is trimmed.
- Blank lines are ignored.
- Lines whose trimmed text starts with `#` are ignored as full-line comments.
- Inline comments are not supported.
- Indentation has no semantic meaning.
- Block header keywords must be lowercase: `share`, `arrange`, `before`, `after`, `end share`, `end arrange`.

### 2.1 Structural assignment delimiter

Structural assignment uses the exact delimiter ` = `: an equals sign with ASCII spaces around it.
This is intentional; it prevents `arrange foo=bar.md` from being silently interpreted as `slug=foo, path=bar.md`.

Valid:

```text
share tpl = snippets/header.rs
arrange main = src/foo.rs
before body = 1-end
```

Invalid:

```text
share tpl=snippets/header.rs
arrange main=src/foo.rs
before body=1-end
```

If an `arrange` target path contains `=`, use a slugged arrange:

```text
arrange target = foo=bar.md
  before all = 1-end
  after  all
end arrange
```

Unslugged arrange paths containing `=` are invalid because they are ambiguous.

### 2.2 Syntax summary

This is a compact syntax summary, not a parser generator grammar.
Semantic checks are listed in later sections.

```text
<document> ::= (<share-block> | <arrange-block>)+

<share-block> ::=
  "share" <space> <identifier> " = " <file>
    <share-entry>+
  "end share"

<share-entry> ::= <identifier> " = " <range>

<arrange-block> ::=
  "arrange" <space> (<file-without-equals> | <identifier> " = " <file>)
    "before" <space> <file-state>
    "after"  <space> <file-state>
  "end arrange"

<file-state> ::= "<missing>" | "<empty>" | <sequence>

# On a `before` line, <sequence> is a <before-sequence>.
# On an `after` line, <sequence> is an <after-sequence>.
<before-sequence> ::= <before-item> ("," <before-item>)*
<after-sequence>  ::= <after-item>  ("," <after-item>)*

<before-item> ::= <range> | <identifier> " = " <range> | <gap-ref>
<after-item>  ::= <range> | <identifier> | <gap-ref> | <external-ref>

<external-ref> ::= <identifier> "::" <identifier>
<gap-ref>      ::= "<gap:" <identifier> ">"

<range> ::= <line> | <line> "-" <line> | <line> "-end"
<line>  ::= positive 1-based integer
```

### 2.3 Identifiers

Identifiers are used for share slugs, arrange slugs, chunk names, and gap names.

Current identifier contract:

```text
[A-Za-z_][A-Za-z0-9_]*
```

Lowercase reserved words are invalid identifiers:

```text
share arrange before after end missing empty gap
```

Hyphens are not valid inside identifiers. Use underscores.

## 3. Path contract

Paths are interpreted relative to the command working directory unless absolute.
Absolute paths are accepted only if their normalized/canonicalized identity remains inside the command working directory.

Path safety rules:

- Existing paths must resolve to files, not directories.
- Missing target paths are allowed for `arrange`; their nearest existing ancestor must be a directory.
- `..`, `.`, platform separators, and symlinks are normalized before containment checks.
- Paths escaping the command working directory are rejected.
- Path identity keys are case-insensitive on Windows and case-sensitive elsewhere.
- One normalized path may appear in at most one `share` block.
- One normalized path may appear in at most one `arrange` block.
- A normalized path may not appear as both `share` and `arrange`.
- Arrange target paths with prefix conflicts are rejected, e.g. `foo` and `foo/bar.rs` in the same document.

If future code cannot determine whether two paths identify the same file, it should fail closed rather than risk ambiguous ownership.

## 4. Pre-state snapshot

A single invocation has one pre-state snapshot.

All `share` ranges and all `arrange before` ranges resolve against that snapshot. No `arrange` block is applied before another block is interpreted.

Example:

```text
arrange main = src/main.rs
  before parser = 20-80, rest = 81-end
  after  rest
end arrange

arrange src/parser.rs
  before <missing>
  after  main::parser
end arrange
```

`main::parser` refers to lines `20-80` from `src/main.rs` before the invocation, not to the file after applying the first arrange block.

## 5. Material model

Material is a slice of logical lines from a pre-state file.

Material can be:

- named local chunk: `name = range` in the current arrange `before`;
- anonymous local range: `range` in the current arrange `before`;
- explicit local gap: `<gap:name>` in the current arrange `before`;
- external named chunk: `slug::name` from a `share` block or another slugged arrange.

Material identity is provenance-based, not text-based. Two chunks with identical text are distinct if they come from different declared ranges.

## 6. Share blocks

A `share` block declares read-only named material:

```text
share <slug> = <file>
  <name> = <range>
end share
```

Rules:

- The source file must exist and be readable text.
- The source file is never modified by the share block.
- Each share entry declares one named range.
- Share entry names must be unique inside the share block.
- Share slugs share one global namespace with slugged arrange blocks.
- A share item is referenced as `<slug>::<name>` from `after`.

## 7. Arrange blocks

An `arrange` block declares one target file transition:

```text
arrange <file>
  before <file-state>
  after  <file-state>
end arrange
```

A slugged arrange also exports named before chunks to other arrange blocks:

```text
arrange <slug> = <file>
  before <name> = <range>, ...
  after  ...
end arrange
```

Rules:

- The target file is the only file this block may create, rewrite, truncate, or delete.
- Only named `before` chunks are exported through `<slug>::<name>`.
- Anonymous ranges and gaps are local-only.
- An arrange block may not reference its own slug in `after`; it must reference its own material locally.
- Arrange slugs share one global namespace with share slugs and other arrange slugs.

## 8. File-state model

A file state is exactly one of:

```text
<missing>
<empty>
<sequence>
```

| State | Meaning |
|---|---|
| `<missing>` | File does not exist. |
| `<empty>` | File exists and has zero bytes. |
| `<sequence>` | File content is a non-empty sequence of declared material. |

`<missing>` and `<empty>` are atomic states. They cannot be mixed with sequence items.
Whitespace-only files are not `<empty>`. A file containing only newlines is not `<empty>`.

Valid transitions:

| Before | After | Meaning |
|---|---|---|
| `<missing>` | `<sequence>` | Create a file with rendered material. |
| `<missing>` | `<empty>` | Create a zero-byte file. |
| `<empty>` | `<sequence>` | Fill an empty file. |
| `<empty>` | `<empty>` | Validate empty file; no content change. |
| `<empty>` | `<missing>` | Delete empty file. |
| `<sequence>` | `<sequence>` | Rewrite file as rendered sequence. |
| `<sequence>` | `<empty>` | Truncate file to zero bytes. |
| `<sequence>` | `<missing>` | Delete file. |

`<missing> -> <missing>` is invalid.

## 9. Complete-before invariant

Every `arrange before` must describe the complete pre-state of its target file.

For an existing non-empty file, `before <sequence>` must cover the whole file:

- the first range starts at line 1;
- the final range reaches EOF;
- ranges are ordered by physical line number;
- ranges do not overlap;
- any non-empty interval between adjacent ranges is represented by an explicit `<gap:name>`;
- no original text is implicit.

Invalid:

```text
arrange src/foo.rs
  before dead = 120-160
  after  <empty>
end arrange
```

Valid:

```text
arrange src/foo.rs
  before prefix = 1-119, dead = 120-160, suffix = 161-end
  after  prefix, suffix
end arrange
```

Numeric EOF guards such as `21-200` are accepted only when they resolve to the actual final line. Prefer `A-end` for EOF ranges.

## 10. Explicit-gap invariant

There are no hidden gaps.

If two declared ranges are not adjacent in the target file, the intervening original text must be declared as `<gap:name>`:

```text
arrange src/foo.rs
  before A = 1-10, <gap:comment>, B = 20-end
  after  B, <gap:comment>, A
end arrange
```

Rules:

- A gap is valid only between two range-bearing before items.
- Empty gaps are invalid.
- Gap names share the current arrange's local namespace with named chunks.
- A gap may be copied, moved, preserved, or deleted by including or omitting it from `after`.
- A gap is never exported through `<slug>::<name>`.

## 11. After-provenance invariant

Every `after` item must resolve to declared material.

Allowed sources:

1. current arrange `before`;
2. `share` named material;
3. another slugged arrange's named `before` chunks.

Disallowed:

- referencing a range that did not appear as an anonymous range in the current `before`;
- referencing a named range by its raw range expression;
- referencing another arrange's anonymous range;
- referencing another arrange's gap;
- referencing the current arrange through its own slug;
- creating arbitrary literal text inside `after`.

This invariant keeps `after` auditable: every final line must have declared provenance.

## 12. Text I/O and rendering

Ranges select whole logical lines, not byte offsets or half-lines.

Input decoding:

- Supported existing-file encodings are UTF-8, UTF-8 BOM, GBK, and Windows-1252.
- Unsupported or invalid text fails with `ENCODING_ERROR` rather than being rewritten through lossy replacement.

Rendering:

- Existing non-empty target rewrites preserve the target file's detected newline style and final-newline convention.
- Newly created non-empty files use LF and a final newline.
- `after <empty>` writes a zero-byte file.
- `after <missing>` deletes the file or leaves it absent.

## 13. Apply safety

Default mode is preview-only. Applying changes requires `--yes`. `--dry-run` overrides `--yes`.

Safety rules:

- The whole document is parsed and semantically validated before target mutation starts.
- Validation failure writes no files.
- Only files declared by `arrange` may be mutated.
- Files declared only by `share` are never modified.
- Missing parent directories for write targets are created during apply.
- Writes are prepared before persistence where practical.
- Deletes are applied after writes.
- If a filesystem error occurs after some targets have already changed, the error must report partial application and list the already affected targets.

This is not a transactional rollback system.

## 14. Preview and JSON output

Preview output is part of the safety surface. It must be predictable and reviewable.

Current compact preview includes:

- invocation state: dry-run, written, or no-op;
- share paths and resolved item line counts;
- target paths and optional slugs;
- before and after summaries;
- exports from slugged arrange blocks;
- explicit gaps and resolved line intervals;
- derived effects;
- final changed/no-write summary.

Current JSON output uses the project output envelope and includes:

- `written`;
- `changed`;
- `shares`;
- `targets`.

Machine-readable output must represent the same semantic model as compact preview. Future preview additions should improve reviewability without changing DSL semantics.

## 15. Derived effects

The DSL does not encode edit actions. Effects are derived from before/after differences.

Current effects are coarse target-level descriptions such as:

- create file;
- delete file;
- create empty file;
- clear file;
- rewrite file;
- no-op.

Future preview code may derive more detailed labels, but those labels must remain informational. They are not DSL primitives.

## 16. Parser policy

The parser should reject likely agent mistakes rather than silently ignoring malformed fragments.

Reject:

- empty document;
- unknown top-level block headers;
- unterminated blocks;
- nested blocks;
- duplicate slugs;
- duplicate names in a local namespace;
- empty sequence items, including dangling commas;
- mixed `<missing>` or `<empty>` with sequence material;
- malformed range expressions;
- malformed material references;
- unspaced structural assignment delimiters;
- ambiguous opener syntax.

## 17. Test obligations

Changes to parser, planner, path handling, text I/O, preview, or apply behaviour should keep coverage for these cases.

### 17.1 Parser

- valid share and arrange blocks;
- slugged arrange;
- full-line comments and blank lines;
- invalid block nesting or missing `end`;
- empty document rejection;
- empty sequence item rejection;
- dangling comma rejection;
- malformed range rejection;
- unspaced `=` rejection;
- ambiguous `arrange foo=bar.md` rejection;
- `arrange slug = foo=bar.md` acceptance.

### 17.2 Path identity and safety

- `./a` and `a` resolve to the same identity;
- duplicate arrange target rejected;
- duplicate share source rejected;
- share/arrange same normalized path rejected;
- arrange target prefix conflict rejected;
- path escape through `..` rejected;
- symlink escape rejected where supported;
- conservative failure when identity cannot be determined.

### 17.3 Before validation

- missing target requires `before <missing>`;
- empty target requires `before <empty>`;
- non-empty target requires full sequence coverage;
- hidden gap rejected;
- explicit non-empty gap accepted;
- empty gap rejected;
- gap at sequence boundary rejected;
- overlapping or out-of-order ranges rejected;
- numeric EOF guard accepted only when it resolves to actual EOF.

### 17.4 After provenance

- local named chunk accepted;
- local anonymous range accepted only if declared anonymously;
- named chunk cannot be referenced by raw range;
- share reference accepted;
- slugged arrange before chunk accepted;
- current arrange self-slug reference rejected;
- external anonymous range rejected;
- external gap rejected;
- undeclared reference rejected.

### 17.5 Text I/O

- LF and CRLF preservation for existing files;
- final-newline preservation for existing files;
- created non-empty file uses LF with final newline;
- `<empty>` creates or preserves zero-byte file;
- undecodable text fails without lossy rewrite.

### 17.6 Apply safety

- default invocation does not write;
- explicit `--yes` writes only arrange targets;
- `--dry-run` overrides `--yes`;
- share files are never modified;
- parent directories are created when needed;
- no writes happen after validation failure;
- partial filesystem failure is reported explicitly.

## 18. Evolution rules

Future extensions should preserve the core model: a reviewable state declaration over known line material.

Acceptable extensions:

- clearer diagnostics;
- richer preview fields;
- machine-readable plan improvements;
- optional hash guards;
- safer apply staging;
- additional input-source conveniences.

Extensions requiring design review:

- literal text insertion;
- implicit range discovery;
- wildcard paths;
- multiple arrange blocks for one file;
- execution-order semantics;
- AST-aware edits;
- automatic import/module repair;
- hidden gap or implicit prefix/suffix behaviour.
