---
revision: 1
date: 2026-06-13T19:28:07
trigger: "review-feedback"
---

# interactive-controls

## Reason

Interactive review found that the MVP selector prompt lacked stable control commands and ignored-file selection controls:

- Windows/PowerShell may pass `^D` as literal input instead of EOF, so relying on Ctrl+D is not portable.
- `help`, `exit`, and related user intents were parsed as invalid sources.
- FZF candidates respected `.gitignore`, preventing selection of ignored files/directories unless manually typed.
- Prefix completion should not block the fix; help/error hints are sufficient for this revision.

## Changes

### Spec Impact

Interactive mode gains internal commands and ignored-file controls:

```text
/help      print interactive command/source help
/list      show selected sources
/done      finish input and render
/exit      exit without rendering
/all       toggle fzf ignored-file inclusion for the current session
^D         literal Ctrl+D text is treated as /done
```

CLI gains:

```text
--no-gitignore    Include files/directories normally hidden by .gitignore in interactive fzf candidates and dir expansion.
```

Explicit source input remains allowed even for gitignored files if the path exists.

### Design Impact

- `interactive.rs` MUST classify slash commands before source parsing.
- Real EOF and literal `^D` both finish input and render.
- `/exit` aborts interactive input without rendering; the command exits successfully without requiring sources.
- FZF candidate providers MUST accept a `respect_gitignore` flag.
- `/all` toggles `respect_gitignore` for subsequent fzf selector lines.
- Prefix completion remains out of scope; `/help` and invalid-input hints list valid prefixes.

### Task Impact

Add feedback tasks for interactive commands, ignored-file controls, tests, docs, and verification.
