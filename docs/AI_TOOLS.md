# AI tools (M20)

The `editor-ai-tools` crate defines the workspace-bound tool surface for coding agents. Each tool:

- Resolves paths with [`canonical_under_workspace`](crates/editor-ai-tools/src/path.rs) so nothing escapes the project root.
- Stages writes in [`WorkspaceTx`](crates/editor-ai-tools/src/transaction.rs); call `commit_all`, `commit_selected`, or `rollback_all`. Disk writes on commit go through [`TextBuffer::apply_edit`](crates/editor-core/src/buffer/mod.rs) for open buffers (undo-correct).
- Supports `dry_run: true` in [`Tool::invoke`](crates/editor-ai-tools/src/tool.rs) where documented below.
- Exposes JSON Schema via [`schemars`](https://docs.rs/schemars) for provider wire format (`ToolDef` in `editor-ai-provider`).

## Configuration (shell only)

Optional `.ide/tools.toml` at the workspace root:

```toml
[shell]
enabled = false
allowed_prefixes = ["cargo", "npm", "pnpm", "yarn", "python", "pytest", "make", "ls", "cat", "echo"]
```

`run_shell` is **off by default**. When enabled, only commands whose text starts with a whitelisted prefix (exact token or `prefix `) run. **Shell runs are not part of the transaction** and cannot be rolled back with `WorkspaceTx`.

## Tools (12)

| Name | Purpose |
|------|---------|
| `read_file` | Read UTF-8 text; optional 1-based `start_line` / `end_line`; **1 MiB max**. |
| `list_directory` | List entries under a path (`recursive`, `max_depth`); gitignore-aware. |
| `find_files` | Glob match under root (max **500** paths). |
| `grep` | Project search via `editor-search` (`is_regex`, `case_sensitive`, optional `path` filter). |
| `edit_lines` | Replace an inclusive **1-based** line range with `new_content` (staged). |
| `insert_at` | Insert `content` **before** line `line` (`line == 1` → start of file). |
| `append_to` | Append at EOF; adds a leading newline if the file has content but no trailing newline. |
| `replace_in_file` | Exact string replace; `occurrence` default `1` (first only), `0` = all. |
| `create_file` | Stage new file; errors if path exists. |
| `delete_file` | Stage delete. |
| `move_file` | Stage rename; destination must not exist. |
| `run_shell` | Optional shell (see above); captures stdout/stderr (100 KiB cap each), `timeout_seconds` default 30. |

## Previews and M23

[`WorkspaceTx::preview_as_diffs`](crates/editor-ai-tools/src/transaction.rs) returns line hunks per path via `editor-diff`, for inline approval UI. `commit_selected` applies pending entries by index after user choice.

## Registry

[`ToolRegistry::new_default`](crates/editor-ai-tools/src/registry.rs) takes `&Arc<Workspace>`, `&Arc<RwLock<BufferManager>>`, `&ToolConfig`, and `Option<Arc<RwLock<SkillRegistry>>>` (`editor-skills`). When `Some`, three extra tools are registered: `load_skill`, `list_skills`, `load_skill_reference` (M27). Pass `None` for the default **12** M20 tools only. Call `as_defs()` for `ChatRequest.tools` and `invoke` to run a validated tool by name.
