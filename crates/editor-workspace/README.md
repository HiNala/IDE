# `editor-workspace`

Headless **workspace** and **multi-buffer** model (mission **M13**).

- **`Workspace`** — project root, `.gitignore` / `.git/info/exclude` rules, recursive `notify` watcher, gitignore-aware file walk (`ignore::WalkBuilder` with `require_git(false)` so rules apply without a `.git/` directory).
- **`BufferManager`** — multiple [`TextBuffer`](https://docs.rs/editor-core) instances with per-buffer cursor, selection, undo, scroll, encoding, and dirty state.

`editor-app` wires [`BufferManager`](src/buffers.rs) and optional [`Workspace`](src/workspace.rs) (see [`docs/WORKSPACE_MODEL.md`](../docs/WORKSPACE_MODEL.md)).

## Tests

```bash
cargo test -p editor-workspace
# Optional: flaky file-watcher smoke (OS-dependent)
cargo test -p editor-workspace -- --ignored
```
