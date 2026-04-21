---
name: using-terminal
description: How to run shell commands through the integrated terminal. Use this skill whenever the task requires executing any shell command — running tests, building code, installing dependencies, inspecting files with standard Unix tools, starting dev servers, or running any CLI tool. Prefer this over any other shell access.
---

# Using the integrated terminal

This IDE has an integrated terminal that runs the user's native shell. The `run_terminal` tool runs commands in that terminal. The user sees exactly what you execute.

## When to use run_terminal vs run_shell

- Prefer `run_terminal(command)` for everything the user would reasonably want to watch happen.
- Use `run_shell` only for commands whose output is pure data the model consumes and where showing the user noise would be worse than useful.

## Interpreting output

`run_terminal` returns captured output and an exit code. Non-zero exit is failure. Read the last ~50 lines carefully — real errors are usually at the end.

## Long-running commands

If you start a server or watcher, the call may time out. Ask the user to start it manually, or use background start (`&` on Unix, `Start-Process` on PowerShell).

## Patterns

- Tests: `run_terminal("cargo test")`, `run_terminal("pytest")`, `run_terminal("npm test")`.
- Ask the user before adding dependencies (`cargo add`, `npm install`, …).

## Never

- Never `rm -rf` without explicit user confirmation.
- Never modify `~/.bashrc` or similar without confirmation.
- Never run commands you do not understand.
