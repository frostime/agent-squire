---
revision: 3
date: 2026-06-13T20:05:00
trigger: "review-feedback"
---

# grouped-fence-clarity

## Reason

Grouped `dir:` and `glob:` output currently nests normal `FILE-START` fences inside `DIR-START`/`GLOB-START`. The text is technically delimited, but visual hierarchy is ambiguous: nested files look like top-level `file:` sources.

Review also requested friendlier interactive help/commands and a minor version bump for the new feature release.

## Changes

### Spec Impact

Nested file fences inside grouped sources MUST include their parent group kind:

```text
====== DIR-FILE-START: src/a.rs ======
...
====== DIR-FILE-END ======

====== GLOB-FILE-START: src/a.rs ======
...
====== GLOB-FILE-END ======
```

Top-level `file:` sources remain:

```text
====== FILE-START: src/a.rs ======
...
====== FILE-END ======
```

Interactive help SHOULD explain commands, selectors, `edit>` confirmation, `/all`, and finishing behavior clearly.

Release metadata SHOULD bump the package minor version to `0.8.0` and update changelog/docs.

### Design Impact

- `template.rs` should render grouped file blocks with `group-file` fence names without changing file body content.
- `interactive.rs` should improve startup/help/status text without introducing unrelated command behavior.
- Version update is metadata-only.

### Task Impact

Add revision tasks for grouped fence rendering, interactive help polish, version/changelog/docs updates, tests, and verification.
