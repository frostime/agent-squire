---
revision: 1
date: 2026-06-10T02:36:08
trigger: "review-feedback"
---

<!-- MUST set trigger to one of: review-feedback | discovery | scope-expansion | correction
This file records scope/design changes after Plan begins (spec/design locked).
Do NOT use revisions during Design phase — edit spec.md/design.md directly.
File naming: revisions/NNN-description.md (incrementing number). -->

# separate-compile-render-phases

## Reason
<!-- The "cause" in the causal chain: trigger source, what problem or new requirement was discovered. -->

Review feedback identified that the implementation does not make the template engine phases explicit enough. `--check` is currently safe because it returns before `render()`, but static semantic normalization lives in the render path, so `--check` does not validate all non-side-effect errors. The code also keeps CLI orchestration, render traversal, and per-interpolation eval glue in `mod.rs`, which makes `parse` / `compile` / `eval` / `render` terminology and responsibilities harder to reason about.

## Changes

### Spec Impact
<!-- Which parts of spec.md logically changed?
Do NOT modify the original spec.md — record the changes here. -->

No user-visible syntax or CLI behavior changes. The logical implementation contract is refined:

- `--check` MUST perform parse + compile/static semantic validation and MUST NOT evaluate sources.
- `--list-sources` MUST read source metadata from the compiled program and MUST NOT evaluate sources.
- Rendered output semantics, default temp output, `--stdout`, `--allow-exec`, and JSON body exclusion remain unchanged.

### Design Impact
<!-- Which parts of design.md changed? Delete this section if no design.md exists. -->

Add explicit terminology and phase boundaries:

| Term | Meaning | Side effects |
|------|---------|--------------|
| `parse` | Template text to AST | No |
| `compile` | AST to compiled program; role normalization and static validation | No |
| `eval` | Evaluate one compiled interpolation | Yes: may read/exec |
| `render` | Evaluate compiled program by joining literals and interpolation eval results | Yes |
| `write` | Persist rendered body to temp/file/stdout | Yes |
| `compose` | CLI orchestration over all phases | Yes |

Refactor module boundaries:

```text
mod.rs       CLI args + top-level orchestration only
parser.rs    syntax parse only
compile.rs   static semantic analysis + source list
render.rs    render_program() + private eval_interpolation()
sources.rs   side-effectful source resolution
modifiers.rs pure text transforms and limit helpers
output.rs    output writing/status/errors
text.rs      decoding and text helpers
model.rs     shared AST/program/error data
```

Move command role normalization out of `modifiers.rs` into `compile.rs`. Keep per-interpolation eval private inside `render.rs` to avoid a shallow `eval.rs` module.

### Task Impact
<!-- Impact on tasks.md: which tasks were added/modified/removed.
tasks.md is a living document — update it directly. -->

Add feedback tasks to `tasks.md`:

- Add `compile.rs` and `render.rs` modules.
- Move normalization/static validation to compile phase.
- Move render loop/eval glue out of `mod.rs`.
- Update tests so `--check` catches compile-time conflicts without executing sources.
- Re-run targeted compose tests, clap test, and clippy.
