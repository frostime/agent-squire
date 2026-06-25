---
name: img-clipboard
status: REVIEW
change-type: single
created: 2026-06-25T17:02:39
reference:
  - source: ".sspec/changes/26-06-25T17-02_img-clipboard/revisions/001-windows-dib-fallback.md"
    type: "revision"
    note: "Windows CF_DIB fallback for clipboard images that arboard cannot convert."
---

# img-clipboard

## Problem Statement

0 Rust CLI clipboard image paths causing screenshot/clipboard images to require the browser-only `imgweb` paste flow before an agent or script can consume a local image path.

Current `imgweb` already supports browser paste upload, but `asq` itself has no system clipboard read/write dependency or command surface. The user needs a shorter CLI path: copy/screenshot an image, run `asq img`, receive a stable local image path.

## Proposed Solution

### Approach

Add a general `img` builtin command and demote `imgweb` to a hidden backwards-compatible legacy command. `img` defaults to reading an image from the system clipboard, saving it as PNG under the existing temp image root, and printing the local path. `img --web` delegates to the current `imgweb` implementation.

Use `arboard` for cross-platform system clipboard access and `image` for PNG encoding. This is preferred over platform shell commands (`powershell Get-Clipboard`, `pbpaste`, `xclip`, etc.) because Agent Squire is a Rust cross-platform CLI and needs predictable errors/output across Windows/macOS/Linux.

### Behavior Contract

**BC-1: New general image CLI**
- Surface: CLI.
- Before: `asq img` is not a built-in command; unknown names fall through to external command mapping.
- After: `asq img` is a built-in command.
- Default: no mode flag means clipboard mode.
- Compatibility boundary: `asq imgweb` continues to start the current local web UI with existing flags and behavior, but is hidden from primary help/list/docs as a legacy entrypoint.

**BC-2: Clipboard image mode**
- Surface: CLI output + generated file.
- Command: `asq img` or `asq img --clipboard`.
- Required behavior: read the current system clipboard image, encode it as PNG, write it under the system temp `agent-temp/images/clip-...` directory, and print the saved path in compact/text/raw/ndjson modes.
- JSON behavior: `asq --json img` prints an `Envelope` with `command: "img"` and data containing `path`, `uri`, `mime`, and `size_bytes`.
- Error behavior: if the clipboard has no image or the platform clipboard cannot be opened/read, exit non-zero with a concise error through the existing CLI error path.
- Boundary: this change does not write anything back to the clipboard and does not parse clipboard file lists.

**BC-3: Web mode through general CLI**
- Surface: CLI.
- Command: `asq img --web [--no-open] [--max-mb <MB>]`.
- Required behavior: start the same local web UI as `asq imgweb`, with equivalent URL, storage, upload, paste, prompt, and shutdown behavior.
- Boundary: no server API or HTML UI behavior changes are required.

### Implementation Changes

**feat(cli): Add `img` builtin command**
- Add `img` to the top-level CLI and builtins registry.
- Serve BC-1 and BC-3.

**chore(cli): Hide legacy `imgweb` entrypoint**
- Keep `asq imgweb` executable but remove it from primary discovery surfaces.
- Serve BC-1 compatibility boundary.

**feat(img): Save clipboard image as PNG**
- Add clipboard image read, PNG encode, temp-file storage, and output formatting.
- Serve BC-2.

**feat(img): Delegate `--web` to existing `imgweb`**
- Reuse `imgweb::run` rather than moving or rewriting the existing server implementation.
- Serve BC-3 and compatibility boundary.

**build(deps): Add clipboard and PNG dependencies**
- Add `arboard` and `image` dependencies for system clipboard image access and PNG encoding.
- Serve BC-2.

**docs(readme): Document `img` clipboard/web usage**
- Update command list and image workflow docs.
- Serve BC-1 through BC-3.

### Scope Summary

| File | Change | Effort |
|---|---|---:|
| `Cargo.toml` / `Cargo.lock` | Add `arboard` and `image` dependencies | S |
| `src/cli.rs` | Add `img` subcommand/dispatch; hide legacy `imgweb` from primary discovery | S |
| `src/builtins/mod.rs` | Register `img` module | XS |
| `src/builtins/img/mod.rs` | New general image command, clipboard save path, output formatting | M |
| `src/builtins/imgweb/mod.rs` | Expose/reuse web defaults if needed; keep `imgweb` behavior stable | XS |
| `README.md` | Document `img` as the public image entrypoint | S |
| `CHANGELOG.md` | Add unreleased/user-visible entry if project format supports it | XS |

### Design Reference

See [design.md](./design.md).
