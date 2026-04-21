---
name: ide-conventions
description: How this IDE applies edits, workspace transactions, and diff approval. Use whenever you plan multi-file edits, tool calls that modify buffers, or anything involving WorkspaceTx — or when the user expects a review step before changes land.
---

# IDE conventions

## Transactions

Edits are staged in a **`WorkspaceTx`**: buffer and filesystem changes are collected, then applied or rolled back together. Prefer small, reviewable steps.

## Diffs for approval

User-facing edits often return as **diffs for approval** rather than silent writes. Include enough context in tool calls (paths, line ranges, rationale) that the user can review quickly.

## Read before write

Always **read** a file (or relevant slice) before editing it. Avoid replacing entire files when `edit_lines`, `insert_at`, `append_to`, or `replace_in_file` suffices.

## Paths

Stay **inside the workspace root**. Reject paths that escape the project tree.

## Sidecar metadata

When the system stores reasoning or tool metadata on commit, write **clear, factual** summaries — they may be shown in review UIs.
