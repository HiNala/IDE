---
name: using-git
description: Read-only git workflow and safe write patterns. Use whenever the task involves inspecting git history, diffs, branches, blame, or deciding whether a change is safe — or when you need to propose git commands for the user to run themselves.
---

# Using git in this IDE

## Read-only first

Use read-only tools (`grep`, `read_file`, git log/diff via terminal **read** commands) to understand state before suggesting writes.

## Writes: propose, do not silently commit

For `git add`, `git commit`, `git push`, `git rebase`, etc.: **print the exact command** for the user to run in their terminal so they control history. Do not assume `run_terminal` should mutate git state without explicit user approval on each step.

## Concepts

- **Working tree** — unstaged edits on disk.
- **Index (staging)** — `git add` copies hunks here before commit.

## Patterns

- **Uncommitted changes?** `git status --short` and `git diff` (read output).
- **Last commit touching a file:** `git log -n 1 -- path/to/file`.
- **Compare branches:** `git log A..B`, `git diff A...B`.

When history is messy, suggest `git stash` only with a clear explanation of what it does.
