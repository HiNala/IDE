# M20 — Agent Tool-Use API & Safe Edit Transactions

**Mission ID:** M20
**Prerequisites:** M19 complete. Provider layer exists and emits `ChatEvent::ToolCall`.
**Output:** A new `editor-ai-tools` crate that defines the structured Rust API an LLM invokes to manipulate the workspace. Tools bound to a specific `Workspace` so an agent cannot escape project bounds. Every write operation flows through a `WorkspaceTx` transaction that can be committed atomically (integrating with the M17 diff renderer for user preview) or rolled back. Tools cover the full set a coding agent needs: read, precise edit (lines, insert, append), create, delete, move, list, find, grep, shell-run (opt-in). Each tool has a JSON Schema compatible with Anthropic / OpenAI tool-use formats. **No UI** — M23 consumes this; M20 is the clean programmatic surface.
**Estimated scope:** 2-3 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 3 agent substrate.
- `/docs/WORKSPACE_MODEL.md` — `Workspace` and `BufferManager` from M13.
- `/docs/TEXT_ENGINE.md` — `TextBuffer::apply_edit` for undo-safe edits.
- M17 — `DiffOverlay` — what the user sees when approving.
- Research context: Anthropic, OpenAI, and Gemini all converge on the same tool-use shape: a tool is defined by `(name, description, json_schema)`; the model emits a call with `name` + `input`; the host executes and returns a `tool_result`. Tool interop across providers is largely free as long as the schema is clean.

---

## The Situation In Plain English

This is the mission where "the agent can actually do things." We're defining the *verbs* an LLM can use on our project: the things it can ask for, the things it can change, the boundaries it can't cross. Get this surface right and every subsequent AI feature — chat, approval flows, background agents later — becomes straightforward. Get it wrong and everything that calls into it inherits the mess.

Two design principles guide M20:

**Bind tools to a Workspace.** Every tool instance is constructed with a reference to the current `Workspace`. The tool validates that any path it touches resolves under the workspace root after canonicalization. An agent asking to `read_file("/etc/passwd")` gets a hard error — not because we manually blocklist system paths, but because the path does not resolve under the workspace root. Path traversal attempts (`../../etc/passwd`) fail the same way. This is a security boundary enforced once, in one place, not sprinkled through each tool's implementation.

**All writes go through transactions.** Brian's explicit guidance: the LLM should edit like a human — precise inserts, line-range replacements, appends, not wholesale file rewrites. The tool set reflects this (`edit_lines`, `insert_at`, `append_to`, not just `write_file`). Every write, no matter which tool produced it, stages into a `WorkspaceTx`. The transaction accumulates a set of pending changes. When the caller (eventually M23's chat panel) is ready, it can either `commit_all` (push every change through `TextBuffer::apply_edit` so undo works correctly), `rollback_all` (discard), or, most importantly, render the pending transaction as a set of inline diffs via M17 so the user can review each hunk and accept/reject individually. That review-and-approve loop is what makes the agent trustworthy in an interactive setting.

The tool set is deliberately not maximal. We ship the specific twelve that matter:

- **Read operations**: `read_file`, `list_directory`, `find_files`, `grep`.
- **Precise write operations**: `edit_lines`, `insert_at`, `append_to`, `replace_in_file`.
- **File management**: `create_file`, `delete_file`, `move_file`.
- **Shell (opt-in)**: `run_shell` — off by default, enabled via per-workspace config.

Each tool has a JSON Schema. Each tool has a Rust function that executes against a `WorkspaceTx`. Each tool supports a `dry_run` flag that returns what would happen without executing. This three-property uniformity is what lets M23 expose all of them to the LLM without special-casing.

---

## Scope

**In scope:**
- `editor-ai-tools` crate with the `Tool` trait, `WorkspaceTx`, and the 12 built-in tools.
- JSON Schema generation for each tool, compatible with Anthropic and OpenAI formats.
- Dry-run support everywhere.
- Path safety (no escape from workspace root).
- Integration with `TextBuffer::apply_edit` so every write is undo-safe.
- Integration with `BufferManager` so tools affecting open buffers update buffer state in place rather than writing around them.
- Integration with `DiffOverlay` so a pending `WorkspaceTx` renders as inline diffs on affected buffers.
- Opt-in shell execution with whitelisted command prefixes (config).

**Out of scope:**
- Network tools (web_search, fetch). Anthropic has a server-side `web_search` tool that works automatically; for our local shell, users can configure their own MCP (Model Context Protocol) servers later — V4+.
- MCP server hosting / client integration (V4+).
- Multi-step tool orchestration (the agent loop itself lives in M23).
- Fine-grained permissions (per-tool allow/deny lists beyond the shell whitelist). V4+.
- Streaming tool output (shell output streams via stdout back to the agent, but we buffer-and-return for V3; interactive sessions are V4+).

---

## North Star

An agent sends a `ToolCall { name: "edit_lines", input: {"path": "src/main.rs", "start_line": 42, "end_line": 45, "new_content": "fn main() {}"} }`. The editor:

1. Looks up the tool by name, validates the input against its schema.
2. Resolves the path; confirms it's inside the workspace.
3. Runs `edit_lines` in *staging* mode against the active `WorkspaceTx`.
4. The tx now contains a pending edit for that file.
5. M23 renders the pending edits as inline diffs; the user approves; the tx commits via `TextBuffer::apply_edit`; one undo reverses the whole thing.

Zero provider-specific code. Zero path-escape possible. Every edit reviewable.

---

## TODO List

### 1. Create `editor-ai-tools` crate

- [ ] 1.1. `cargo new --lib crates/editor-ai-tools`. Deps: `serde`, `serde_json`, `schemars` (for JSON Schema derivation), `thiserror`, `anyhow`, `tracing`, `editor-core`, `editor-workspace`, `editor-search`, `editor-diff`, `globset`.
- [ ] 1.2. Commit: `feat(ai-tools): scaffold editor-ai-tools crate`.

### 2. Core `Tool` trait

- [ ] 2.1. `src/tool.rs`:
  ```rust
  #[async_trait::async_trait]
  pub trait Tool: Send + Sync {
      fn name(&self) -> &str;
      fn description(&self) -> &str;
      fn input_schema(&self) -> serde_json::Value;
      async fn invoke(
          &self,
          input: serde_json::Value,
          tx: &mut WorkspaceTx,
          dry_run: bool,
      ) -> Result<ToolOutput, ToolError>;
  }
  pub struct ToolOutput {
      pub content: String,       // human-readable result
      pub structured: Option<serde_json::Value>,   // optional machine-parseable
      pub is_error: bool,
  }
  ```
- [ ] 2.2. Commit: `feat(ai-tools): Tool trait and output types`.

### 3. `WorkspaceTx` — the transaction layer

- [ ] 3.1. `src/transaction.rs`:
  ```rust
  pub struct WorkspaceTx {
      workspace_root: PathBuf,
      buffers: Arc<RwLock<BufferManager>>,
      pending: Vec<PendingChange>,
  }
  pub enum PendingChange {
      EditBuffer { buffer_id: BufferId, edit: BufferEdit },
      WriteNewFile { path: PathBuf, contents: String },
      DeleteFile { path: PathBuf, prior_contents: Option<String> },
      MoveFile { from: PathBuf, to: PathBuf },
  }
  pub enum BufferEdit {
      ReplaceRange { byte_range: Range<usize>, new_text: String },
      InsertAt { byte_offset: usize, text: String },
      FullReplace { new_text: String },
  }
  impl WorkspaceTx {
      pub fn new(workspace_root: PathBuf, buffers: Arc<RwLock<BufferManager>>) -> Self;
      pub fn canonical_path(&self, relative: &str) -> Result<PathBuf, ToolError>;
      pub fn stage_change(&mut self, change: PendingChange);
      pub fn pending_changes(&self) -> &[PendingChange];
      pub fn commit_all(&mut self) -> Result<(), ToolError>;
      pub fn commit_selected(&mut self, indices: &[usize]) -> Result<(), ToolError>;
      pub fn rollback_all(&mut self);
      pub fn preview_as_diffs(&self) -> Vec<(PathBuf, Vec<Hunk>)>;
  }
  ```
- [ ] 3.2. `canonical_path`: resolves `relative` against `workspace_root`, calls `canonicalize`, verifies the result is still under `workspace_root`. Rejects `..` traversal and symlinks-out.
- [ ] 3.3. `commit_all` iterates `pending`, routing each through `BufferManager::get_mut().buffer.apply_edit` for buffer edits (so undo works) or `editor_io::atomic_save` for disk-only operations (creating a file that isn't yet an open buffer).
- [ ] 3.4. `preview_as_diffs` applies pending changes to snapshots, computes `editor-diff::compute_line_diff` vs the current, returns hunks per affected path. This is what M23 shows to the user.
- [ ] 3.5. Commit: `feat(ai-tools): WorkspaceTx with staged changes and path safety`.

### 4. `read_file`

- [ ] 4.1. `src/tools/read_file.rs`:
  - Input: `{ path: string, start_line?: u64, end_line?: u64 }`.
  - Behavior: resolve path; if open in BufferManager, use the in-memory version; else read from disk via `editor-io`. Apply line range if given.
  - Output: file contents, with a header line like `<file path="src/main.rs" lines="1-42">`.
- [ ] 4.2. Size cap: reject files over 1 MB with an error pointing to `grep` for large files. Prevents accidentally dumping huge binaries into agent context.
- [ ] 4.3. Commit: `feat(ai-tools): read_file tool`.

### 5. `list_directory`

- [ ] 5.1. Input: `{ path: string, recursive?: bool, max_depth?: u32 }`.
- [ ] 5.2. Behavior: walk the directory (respecting workspace ignore rules). Default non-recursive.
- [ ] 5.3. Output: JSON array of `{name, type, size}` entries.
- [ ] 5.4. Commit: `feat(ai-tools): list_directory tool`.

### 6. `find_files`

- [ ] 6.1. Input: `{ pattern: string }` (glob syntax).
- [ ] 6.2. Behavior: use `globset` + ignore-aware walk, return matching paths.
- [ ] 6.3. Cap at 500 results.
- [ ] 6.4. Commit: `feat(ai-tools): find_files tool`.

### 7. `grep`

- [ ] 7.1. Input: `{ query: string, is_regex?: bool, case_sensitive?: bool, path?: string }`.
- [ ] 7.2. Behavior: delegate to `editor-search::start_project_search`, collect results.
- [ ] 7.3. Output: structured list of matches with line context.
- [ ] 7.4. Commit: `feat(ai-tools): grep tool via editor-search`.

### 8. `edit_lines` — the precision edit primitive

- [ ] 8.1. Input: `{ path: string, start_line: u64, end_line: u64, new_content: string }` (1-indexed line numbers; end_line inclusive).
- [ ] 8.2. Behavior: find or create the buffer for path. Compute the byte range of lines `start_line..=end_line`. Stage a `BufferEdit::ReplaceRange`.
- [ ] 8.3. Do NOT execute the edit in non-dry mode — stage it in the tx. Execution happens on commit.
- [ ] 8.4. Output: dry-run preview — "Will replace lines 42-45 (3 lines, 87 bytes) with new_content (2 lines, 45 bytes)."
- [ ] 8.5. Commit: `feat(ai-tools): edit_lines precision edit`.

### 9. `insert_at`

- [ ] 9.1. Input: `{ path: string, line: u64, content: string }`.
- [ ] 9.2. Behavior: insert `content` *before* the given line (so `line=1` prepends to the file).
- [ ] 9.3. Stage as `BufferEdit::InsertAt { byte_offset: line_start_offset }`.
- [ ] 9.4. Commit: `feat(ai-tools): insert_at tool`.

### 10. `append_to`

- [ ] 10.1. Input: `{ path: string, content: string }`.
- [ ] 10.2. Behavior: stage append at EOF. If the file doesn't end with a newline, prepend one to the appended content.
- [ ] 10.3. Commit: `feat(ai-tools): append_to tool`.

### 11. `replace_in_file`

- [ ] 11.1. Input: `{ path: string, old_text: string, new_text: string, occurrence?: u32 }`. `occurrence` defaults to 1 (first match); `0` means all.
- [ ] 11.2. Behavior: exact-string search (not regex); require unambiguous match unless `occurrence=0`. Stage the replacement.
- [ ] 11.3. Error cases: `old_text not found` (hard error with a suggestion to run `grep` first); `old_text found N times but occurrence=1` (ambiguous error).
- [ ] 11.4. This is the agent's best tool for "find this exact snippet and replace it" — cleaner than line numbers for most real edits.
- [ ] 11.5. Commit: `feat(ai-tools): replace_in_file tool`.

### 12. `create_file`, `delete_file`, `move_file`

- [ ] 12.1. `create_file`: `{ path: string, content: string }`. Reject if the file already exists. Stage `WriteNewFile`.
- [ ] 12.2. `delete_file`: `{ path: string }`. Read the current contents (for rollback), stage `DeleteFile { prior_contents }`.
- [ ] 12.3. `move_file`: `{ from: string, to: string }`. Both paths must be in workspace. Stage `MoveFile`.
- [ ] 12.4. Commit: `feat(ai-tools): file management tools`.

### 13. `run_shell` (opt-in)

- [ ] 13.1. Input: `{ command: string, cwd?: string, timeout_seconds?: u32 }` (default timeout 30).
- [ ] 13.2. Behavior: validate against a whitelist of allowed command prefixes from `.ide/tools.toml`:
  ```toml
  [shell]
  enabled = false
  allowed_prefixes = ["cargo", "npm", "pnpm", "yarn", "python", "pytest", "make", "ls", "cat", "echo"]
  ```
- [ ] 13.3. If disabled or prefix not allowed → return error immediately without executing.
- [ ] 13.4. Execute via `std::process::Command`. Capture stdout + stderr (cap at 100 KB each). Return exit code + output.
- [ ] 13.5. **This is the only tool that has real-world side effects the transaction system can't roll back.** Document prominently that shell commands are non-transactional.
- [ ] 13.6. Commit: `feat(ai-tools): run_shell with allow-list`.

### 14. `ToolRegistry`

- [ ] 14.1. `src/registry.rs`:
  ```rust
  pub struct ToolRegistry {
      tools: HashMap<String, Arc<dyn Tool>>,
  }
  impl ToolRegistry {
      pub fn new_default(workspace: Arc<Workspace>, buffers: Arc<RwLock<BufferManager>>, config: &ToolConfig) -> Self;
      pub fn as_defs(&self) -> Vec<ToolDef>;  // for provider's ChatRequest
      pub async fn invoke(&self, name: &str, input: serde_json::Value, tx: &mut WorkspaceTx, dry_run: bool) -> Result<ToolOutput, ToolError>;
  }
  ```
- [ ] 14.2. `as_defs()` returns the `ToolDef`s M19 expects in the `ChatRequest.tools` field.
- [ ] 14.3. Commit: `feat(ai-tools): tool registry`.

### 15. Integration with diff renderer

- [ ] 15.1. `WorkspaceTx::preview_as_diffs` produces `Vec<(PathBuf, Vec<Hunk>)>`. M23 will iterate and apply a `DiffOverlay::Inline { hunks }` to each affected buffer.
- [ ] 15.2. When the user accepts a specific hunk, M23 calls `WorkspaceTx::commit_selected(&[hunk_index])` — the transaction applies only those changes.
- [ ] 15.3. Integration sketched; full wiring happens in M23.
- [ ] 15.4. Commit: `feat(ai-tools): preview_as_diffs for M23 integration`.

### 16. Path safety tests

- [ ] 16.1. Attempt to read `../../etc/passwd` → rejected.
- [ ] 16.2. Attempt to read `/absolute/path/outside/workspace` → rejected.
- [ ] 16.3. Symlink inside workspace pointing outside → rejected (canonicalize catches it).
- [ ] 16.4. Normal relative path → accepted.
- [ ] 16.5. These tests are non-negotiable; every one of them must pass before the shell tool even ships.
- [ ] 16.6. Commit: `test(ai-tools): path-safety tests`.

### 17. JSON Schema generation

- [ ] 17.1. Each tool's input struct uses `#[derive(JsonSchema)]` from `schemars`. The schema is generated at runtime and returned from `input_schema()`.
- [ ] 17.2. Verify the generated schemas are accepted by Anthropic (`strict: true` tolerated) and OpenAI.
- [ ] 17.3. Commit: `feat(ai-tools): JSON Schema generation via schemars`.

### 18. Quality gates + documentation

- [ ] 18.1. Standard gates.
- [ ] 18.2. Write `/docs/AI_TOOLS.md` documenting each tool, its schema, and examples.
- [ ] 18.3. Tag: `git tag -a m20-complete -m "M20 complete: agent tool-use API and transactions"`; push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. All 12 tools implemented with tests.
3. Path safety tests all pass.
4. JSON Schemas accepted by Anthropic and OpenAI tool-use validators.
5. `WorkspaceTx::preview_as_diffs` produces correct hunks.
6. `commit_all` routes through `TextBuffer::apply_edit` — one undo step reverses the whole tx.
7. `m20-complete` tag pushed.

## Testing Requirements

- Unit tests per tool, per error case.
- Integration test: 10-tool sequence executing and rolling back cleanly.
- Path safety tests.

## Git Commit Strategy

15-18 commits. Push after items 3, 7, 12, 14, 18.

## Handoff to M21

M21 (metadata sidecar) is about capturing the *why* of agent work. M20 gives agents the ability to *do* things; M21 captures what they did and why, so the knowledge persists.

---

## Standing Orders Reminder

- Every write goes through `TextBuffer::apply_edit`. There is no shortcut, no special case. Undo is non-negotiable.
- Path traversal is a security bug. If you find any code that bypasses `canonical_path`, delete that code.
- The shell tool is dangerous. It is opt-in, prefix-whitelisted, and non-transactional. Document this every time it appears in user-facing docs.
- Schema drift between what Anthropic and OpenAI accept is a real concern. When in doubt, test against both.

Go.
