# Patch-edit compatibility notes

The Rust `patch-edit` implementation is intentionally modeled after the Python script's algorithm.

Important behavioral points:

1. Patch blocks are extracted with the LRR role scan:
   `F B* S [BC]* D [BC]* R`.
2. Markers must be exact standalone lines.
3. `SEARCH` content cannot be empty for non-empty files.
4. Exact line-block matching is attempted before loose matching.
5. Loose matching ignores trailing spaces/tabs and normalizes blank lines.
6. Same-file search patches are matched against original file content first.
7. Same-file matches are rejected if ranges overlap.
8. Replacement is spliced from bottom to top after all matches succeed.
9. `CREATE` upper blocks must be whitespace-only.
10. `OVERWRITE` upper blocks must be whitespace-only.
