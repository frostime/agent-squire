---
revision: 1
date: 2026-06-28T23:07:28
trigger: "review-feedback"
---

# review fixes + DSL grammar formalization

## Reason

Independent code review (subagent: deepseek-v4-pro xhigh, `code-reviewer` preset)
on commit `9a53a87`, plus user concern that the `--prompt` DSL definition is
loosely specified and ambiguous. Core planner verified correct (16 runtime
scenarios PASS, 10/10 tests, clippy clean), but 4 conformance defects and 5 DSL
ambiguities surfaced. All findings spot-checked against source by the main agent
before acceptance.

## Changes

### Spec Impact

Logical changes to spec.md (recorded here; spec.md not edited):

- **BC-5 (JSON output)** — `data` MUST actually carry `chunks` and `action`
  structured fields, not only `written/changed/file/diff`. The original BC-5
  text already promised `file/chunks/action/written/diff`; implementation
  omitted `chunks`/`action`. This revision treats the promise as binding.
- **BC-4 (error codes)** — add `NON_EMPTY_GAP` to the code set. `gap=error` with
  a non-empty inter-slot gap is a semantic validation failure, distinct from
  `INVALID_SPEC` (which design §7 scopes to syntax / missing-or-duplicate
  `file`). Duplicate chunk names in a `rearrange` list now report
  `REARRANGE_SET_MISMATCH` rather than silently passing the multiset check.
- **BC-1 (dry-run/--yes gate)** — `--dry-run` becomes a binding explicit
  override: when both `--dry-run` and `--yes` are passed, `--dry-run` wins (no
  write). Previously `--dry-run` was parsed but ignored.

### Design Impact

Changes to design.md (recorded here; design.md not edited):

1. **§7 error-code table** — add row `NON_EMPTY_GAP` → "gap=error and a
   non-empty gap exists between declared slots". Add: duplicate names in
   `from`/`to` → `REARRANGE_SET_MISMATCH`.

2. **§1 CLI** — `--dry-run` semantics clarified: explicit `--dry-run` forces
   preview even if `--yes` is present. Decision rule: `write = args.yes && !args.dry_run`.

3. **§6 output / Mock** — `--yes` on a no-op (resulting in no change) MUST NOT
   render `(dry-run)`. Render `(no-op)` with the existing "No change." line so
   the user is not misled into thinking `--yes` was missing.

4. **JSON `data` shape** — formalize:
   ```json
   {
     "written": false, "changed": true, "file": "README.md",
     "chunks": { "A": {"range":"1-30","lines":30} },
     "action": { "type":"rearrange", "from":["A","B"], "to":["B","A"], "gap":"slot" },
     "diff": "--- a/README.md\n..."
   }
   ```
   `chunks` present only when the spec declares chunks; `action` always present,
   shape varies by action type (move/copy/delete carry `range`+`anchor`;
   rearrange carries `from`/`to`/`gap`).

5. **NEW §2.1 — DSL grammar (formalized EBNF + lexical rules)**. The original
   §2 prose left these ambiguous; the review found concrete failure inputs:

   - **Region disambiguation by leading digit** (`parser.rs:128`): a token whose
     first char is an ASCII digit is parsed as an inline range, else as a chunk
     name. Consequence: a chunk named with a leading digit (e.g. `1A`) can never
     be referenced. **Resolution**: constrain chunk names to
     `[A-Za-z_][A-Za-z0-9_]*` (identifier rule). Leading-digit names become
     illegal at declaration time → no silent mis-parse.
   - **`to` is a reserved separator** in `move`/`copy` (split on `" to "`): a
     chunk name `to`, or containing ` to `, breaks parsing. **Resolution**: the
     identifier rule forbids spaces; reserve `to`, `file`, `chunk`, `move`,
     `copy`, `delete`, `rearrange`, `start`, `end`, `before`, `after`, `gap` as
     keywords that cannot be chunk names.
   - **Name charset undefined**: names with `,` break rearrange lists; `=` breaks
     chunk declarations. **Resolution**: identifier rule resolves both.
   - **`#` comments are line-leading only**; inline `#` is not stripped.
     **Resolution**: state this explicitly in the grammar.

   Proposed EBNF (to embed in design §2.1 and mirror in `prompt.md`):
   ```ebnf
   spec        = { line } ;
   line        = blank | comment | file-decl | chunk-decl | action ;
   comment     = "#" , { any } ;                  (* whole line only *)
   file-decl   = "file" , ws , path ;             (* exactly one per spec *)
   chunk-decl  = "chunk" , ws , name , "=" , range ;
   action      = move | copy | delete | rearrange ;  (* exactly one per spec *)
   move        = "move"   , ws , region , ws , "to" , ws , anchor ;
   copy        = "copy"   , ws , region , ws , "to" , ws , anchor ;
   delete      = "delete" , ws , region ;
   rearrange   = "rearrange" , ws , namelist , "=>" , namelist , [ ws , "gap=" , gap ] ;
   region      = range | name ;                   (* range if first char is digit *)
   range       = number | number , "-" , number ; (* 1-based inclusive, A<=B *)
   anchor      = "start" | "end" | "before" ws number | "after" ws number ;
   namelist    = name , { "," , name } ;
   gap         = "slot" | "drop" | "error" ;
   name        = ( letter | "_" ) , { letter | digit | "_" } ;  (* not a keyword *)
   number      = nonzero-digit , { digit } ;      (* >= 1 *)
   ```

### Task Impact

Added to tasks.md as a Feedback Tasks block (see that file). Summary:
- Fix JSON `data`: serialize `chunks` + `action` (output.rs).
- Add `NON_EMPTY_GAP` code (model.rs); use it in `gap=error` path (plan.rs).
- Reject duplicate names → `REARRANGE_SET_MISMATCH` (plan.rs validate_set).
- Honor `--dry-run` override (mod.rs).
- No-op under `--yes` renders `(no-op)` not `(dry-run)` (output.rs).
- Enforce identifier rule + keyword reservation for chunk names (parser.rs).
- Embed formalized EBNF in `prompt.md` and design §2.1.
- Extend `tests/rearrange.rs`: JSON has chunks/action; NON_EMPTY_GAP; duplicate
  name; `--dry-run --yes` no write; no-op label; leading-digit/keyword name
  rejected.
