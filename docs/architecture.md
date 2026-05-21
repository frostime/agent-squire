# Architecture

Agent Squire uses a narrow runtime and vertical built-in command modules.

```text
src/
  cli.rs
  runtime/
  external.rs
  builtins/
    tree/
    info/
    toc/
    patch_edit/
```

The runtime owns only cross-cutting mechanics:

- input source resolution
- output mode / JSON envelope
- config loading
- command context

Each built-in command owns its CLI args, execution logic, output shaping, and tests.
