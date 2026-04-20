# M23 — AI Chat Panel & Edit-Approval Flow

**Mission ID:** M23
**Prerequisites:** M22 complete. Provider, tools, metadata, and vector index are all working.
**Output:** The minimum viable integrated AI experience. A right-side chat panel (`Ctrl+L` to toggle) showing conversation turns with streaming token rendering. A composer at the bottom with `@file`, `@selection`, `@workspace` context pins. Provider + model picker in the header. Agent edits proposed through M20 tools surface as M17 inline diffs with per-hunk accept / reject. After commit, M21's summarizer updates sidecars. Token accounting visible. Cancellation at any point. This is the first time every V3 piece comes together in a surface a user can actually use.
**Estimated scope:** 2-3 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 4 integration.
- M19 — `AiProvider`, `ChatRequest`, `ChatEvent`.
- M20 — `ToolRegistry`, `WorkspaceTx`.
- M21 — `SessionLog`, `MetadataUpdater`.
- M22 — `Indexer::retrieve`.

---

## The Situation In Plain English

This is the capstone mission for V3's Ring 4. Every piece we've built gets wired into one user-visible surface: a chat panel where a person asks a question, an agent reasons and acts, proposed edits appear as inline diffs, the user approves or rejects per hunk, committed changes become sidecar knowledge. All the V3 bets — provider abstraction, tool-use safety, metadata capture, vector retrieval — converge here.

The UI is deliberately minimal. A right-side vertical panel, roughly 400 logical pixels wide by default (resizable splitter). Header: provider + model picker, token counter, settings gear. Body: a scrolling conversation log with user messages, assistant messages (streaming as they arrive), tool calls (collapsed by default, expandable to see inputs/outputs), edit proposals as inline diff summaries that open into the actual buffer. Footer: multi-line composer with `@` mentions for context pinning, submit and cancel buttons.

What the loop looks like in practice:

1. User types a prompt. Presses `Ctrl+Enter` to submit.
2. The panel assembles context: user's `@` pins + M22 retrieval against the user's prompt + pinned sidecar summaries + any currently-open-buffer snippets near the cursor.
3. Panel calls `provider.chat(ChatRequest { messages: ... + context, tools: registry.as_defs() })`.
4. Events stream back. Text deltas render in the assistant message. Tool calls get displayed collapsed; when a tool fires (read_file, grep, etc.), it executes against the workspace (read-only tools immediately, write tools via `WorkspaceTx` staging).
5. When the agent finishes, any pending `WorkspaceTx` changes render as M17 inline diffs on the affected buffers. A small banner at the top of the chat panel says "3 files with proposed edits — review."
6. User reviews, approves hunks (keyboard: `y` to accept, `n` to reject, `a` to accept all), rejects others. Accepted hunks commit through the undo-safe path from M20.
7. `MetadataUpdater` fires asynchronously, summarizing the session and updating sidecars for all affected files.
8. The next turn starts with richer context available.

The agent loop (iterating `provider.chat` + tool execution until the model stops calling tools) is the most complex piece. We implement it correctly here so it can be reused by future missions — it's the same loop any agent architecture needs.

---

## Scope

**In scope:**
- `editor-chat` crate with UI + agent loop.
- Right-side panel rendering.
- Conversation model: user messages, assistant messages, tool calls, edit proposals.
- Streaming token rendering.
- Context pinning via `@file`, `@selection`, `@workspace`.
- Automatic context retrieval via M22.
- Provider + model picker.
- Agent loop with tool-call execution.
- Edit proposal → diff overlay → accept/reject → commit → sidecar update.
- Cancellation at any point in the agent loop.
- Token counter display.
- Conversation persistence (chat history saved between sessions in `.ide/chats/`).

**Out of scope:**
- Multi-agent / parallel agent execution (V4+).
- Background agents (V4+).
- Voice input, file drag-drop, image inputs (V4+).
- Rich markdown rendering inside chat messages beyond basic styling (V4+; plain text + simple code blocks for V3).
- Syntax highlighting inside chat code blocks (V4+).
- @url web fetches (V4+; Anthropic has server-side web_search tool that works automatically in some setups).
- Shared team conversations (V4+).

---

## North Star

Open the IDE. `Ctrl+L`. Chat panel slides in from the right. Type: "Add a rate limiter to the login handler." Submit. Watch tokens stream. See the agent call `grep` for `login_handler`, then `read_file` on `src/auth.rs`, then propose edits. The diff appears inline on `src/auth.rs`. Review the 3 hunks. Accept 2, reject 1. The accepted hunks commit. One undo key reverts. Sidecars update quietly in the background. The whole experience feels like a colleague on the other side of the screen who can actually read and write your code.

---

## TODO List

### 1. Create `editor-chat` crate

- [ ] 1.1. `cargo new --lib crates/editor-chat`. Deps: `serde`, `serde_json`, `tokio`, `futures`, `chrono`, `uuid`, `editor-core`, `editor-workspace`, `editor-ui`, `editor-render`, `editor-diff`, `editor-ai-provider`, `editor-ai-tools`, `editor-metadata`, `editor-index`.
- [ ] 1.2. Commit: `feat(chat): scaffold editor-chat crate`.

### 2. Conversation model

- [ ] 2.1. `src/model.rs`:
  ```rust
  pub struct Conversation {
      pub id: String,
      pub title: String,
      pub turns: Vec<Turn>,
      pub created_at: DateTime<Utc>,
      pub provider_id: String,
      pub model: String,
  }
  pub struct Turn {
      pub role: Role,
      pub blocks: Vec<TurnBlock>,
  }
  pub enum Role { User, Assistant, Tool }
  pub enum TurnBlock {
      Text(String),                               // streaming accumulates here
      ContextPin { kind: PinKind, target: String },
      ToolCall { id: String, name: String, input: serde_json::Value, output: Option<String>, is_error: bool },
      EditProposal { tx_preview: Vec<(PathBuf, Vec<Hunk>)>, accepted_hunks: HashSet<(PathBuf, usize)>, rejected_hunks: HashSet<(PathBuf, usize)> },
      Usage { input_tokens: u32, output_tokens: u32 },
  }
  pub enum PinKind { File, Selection, Workspace, Symbol }
  ```
- [ ] 2.2. Commit: `feat(chat): conversation data model`.

### 3. Agent loop

- [ ] 3.1. `src/agent_loop.rs`:
  ```rust
  pub struct AgentLoop {
      provider: Arc<dyn AiProvider>,
      tools: Arc<ToolRegistry>,
      indexer: Arc<Indexer>,
      buffers: Arc<RwLock<BufferManager>>,
  }
  impl AgentLoop {
      pub async fn run_turn(
          &self,
          conversation: &mut Conversation,
          user_input: String,
          pins: Vec<PinKind>,
          cancel: CancellationToken,
          events: Sender<AgentLoopEvent>,
      ) -> Result<(), AgentLoopError>;
  }
  pub enum AgentLoopEvent {
      AssistantTextDelta(String),
      ToolStarted { id: String, name: String, input: serde_json::Value },
      ToolFinished { id: String, output: String, is_error: bool },
      EditProposal(WorkspaceTx),
      Done { usage: Usage, stop_reason: StopReason },
      Error(String),
  }
  ```
- [ ] 3.2. Implementation sketch:
  1. Build context: retrieve top-K from index based on `user_input`; add pin-specified files/selections; add current open buffer if nothing pinned.
  2. Build `ChatRequest` with tools + context + conversation history.
  3. Loop:
     - Call `provider.chat()`.
     - Stream events to the channel.
     - On `ChatEvent::ToolCall`: invoke via `ToolRegistry` (read tools execute immediately; write tools stage in the loop-local `WorkspaceTx`).
     - On `ChatEvent::Done` with `stop_reason = ToolUse`: build a new request appending tool_result messages, loop again.
     - On `ChatEvent::Done` with any other stop reason: break.
  4. After loop: if `WorkspaceTx` has pending changes, emit `EditProposal`. Else done.
- [ ] 3.3. Cancellation: the `CancellationToken` is checked between stream events and between loop iterations. Cancelling mid-stream drops the tx (rollback).
- [ ] 3.4. Iteration cap: 25 tool-use rounds max per turn. Beyond that, emit error ("agent loop exceeded limits"). Prevents runaway loops.
- [ ] 3.5. Commit: `feat(chat): agent loop with tool-use iteration`.

### 4. Panel UI

- [ ] 4.1. `src/panel.rs`:
  ```rust
  pub struct ChatPanel {
      pub visible: bool,
      pub width: f32,
      pub conversation: Conversation,
      pub composer: ComposerState,
      pub scroll: f32,
      pub active_loop: Option<RunningTurn>,
  }
  ```
- [ ] 4.2. Rendering: a vertical panel on the right. Header (provider/model + token counter). Conversation scroll area. Composer at the bottom.
- [ ] 4.3. Each Turn renders as a card: role indicator (small left stripe), text blocks, tool-call summaries, edit-proposal banners.
- [ ] 4.4. Streaming text updates in place — no flicker. Cosmic-text's incremental reshape handles this cleanly.
- [ ] 4.5. `Ctrl+L` toggles visibility. Settings-configurable default width.
- [ ] 4.6. Commit: `feat(chat): ChatPanel base rendering`.

### 5. Composer

- [ ] 5.1. Multi-line input at the bottom. Enter inserts newline; `Ctrl+Enter` (or `Shift+Enter` — pick the VS-Code-compatible one: `Ctrl+Enter`) submits.
- [ ] 5.2. Typing `@` pops a small picker: `@file`, `@selection`, `@workspace`, `@symbol`. Arrow keys + Enter select; tab to confirm; each inserts a pill.
- [ ] 5.3. Pills render as colored chips inside the composer input; can be removed with backspace.
- [ ] 5.4. Submit button (top-right) + Cancel button when a turn is running.
- [ ] 5.5. Commit: `feat(chat): composer with @ pin mentions`.

### 6. Context pinning logic

- [ ] 6.1. `@file` → user picks a file from the workspace (a small palette, same pattern as M14's quick-open). Pins that file's current content into the next turn.
- [ ] 6.2. `@selection` → grabs the current selection from the active buffer if present.
- [ ] 6.3. `@workspace` → tells the agent loop to do an aggressive M22 retrieval against the user prompt (top-20 chunks instead of default top-5).
- [ ] 6.4. `@symbol` → fuzzy-match over the index's code definitions; pick one, pin its source range.
- [ ] 6.5. Commit: `feat(chat): context pinning resolution`.

### 7. Edit-proposal UI

- [ ] 7.1. When an `EditProposal` event arrives, render a card inside the conversation: "3 files with proposed edits: auth.rs (+12/-3), tests.rs (+8/0), Cargo.toml (+1/0). [Review changes]."
- [ ] 7.2. Clicking "Review changes" applies a `DiffOverlay::Inline` to each affected buffer and auto-focuses the first affected buffer. The user sees the diff inline.
- [ ] 7.3. Per-hunk hotkeys while in review mode:
  - `y` — accept current hunk, move to next.
  - `n` — reject current hunk, move to next.
  - `a` — accept all remaining hunks.
  - `d` — reject all remaining hunks.
  - `Esc` — pause review (re-openable later).
- [ ] 7.4. When review is complete (no pending hunks): commit the accepted ones via `WorkspaceTx::commit_selected`. Clear the overlay. Surface a small banner: "3 hunks applied (2 accepted, 1 rejected). Undo to revert."
- [ ] 7.5. Commit: `feat(chat): edit-proposal review and per-hunk approval`.

### 8. Sidecar update on commit

- [ ] 8.1. After the commit succeeds, construct a `SessionLog` from the `Conversation`'s current turn + the committed paths.
- [ ] 8.2. Fire `MetadataUpdater::update_for_session` asynchronously. Show a small "updating knowledge..." indicator; disappear when done.
- [ ] 8.3. Summarizer failure (e.g., Ollama not running) doesn't block — log and move on.
- [ ] 8.4. Commit: `feat(chat): sidecar update on commit`.

### 9. Conversation persistence

- [ ] 9.1. Each conversation saved to `.ide/chats/<id>.json` on every turn completion.
- [ ] 9.2. On panel open: list recent conversations (last 10) in a dropdown in the header. Allow "+ New chat."
- [ ] 9.3. Large conversations (> 100 turns) truncate older turns from the saved file but keep them in-memory for the session.
- [ ] 9.4. Commit: `feat(chat): conversation persistence to disk`.

### 10. Provider + model picker

- [ ] 10.1. Header dropdown: lists all enabled providers and their models. Switching mid-conversation is allowed (recorded in the conversation metadata).
- [ ] 10.2. A small gear icon opens a settings overlay for: max_tokens, temperature, which providers are enabled, API keys (redirect to OS keychain manager).
- [ ] 10.3. Commit: `feat(chat): provider/model picker + settings`.

### 11. Token counter

- [ ] 11.1. Displays cumulative tokens used in the current conversation: `in 8.2K / out 2.1K / total 10.3K`.
- [ ] 11.2. When a provider publishes prices, optionally show estimated cost. Keep this optional — prices change; a stale price is worse than no price.
- [ ] 11.3. Commit: `feat(chat): token counter in panel header`.

### 12. Cancellation

- [ ] 12.1. The active turn's `CancellationToken` is wired to the Cancel button and to `Ctrl+.`. Cancelling:
  - Aborts the provider stream.
  - Rolls back the pending tx.
  - Marks the assistant message as "[cancelled by user]" and preserves partial text already streamed.
- [ ] 12.2. Commit: `feat(chat): cancellation mid-turn`.

### 13. Minimum styling and polish

- [ ] 13.1. Follow V2's "Dark Glass / Refined Civic" aesthetic: dark background, blue accents, zinc greys, Geist font.
- [ ] 13.2. Tool-call blocks have a subtle monospace accent background.
- [ ] 13.3. Edit proposals get amber accent bars.
- [ ] 13.4. No emoji, no cute animations. Smooth scroll, smooth token appearance.
- [ ] 13.5. Commit: `polish(chat): visual style pass`.

### 14. End-to-end integration test

- [ ] 14.1. Scripted test: start Ollama, open a small fixture workspace, submit a known prompt ("rename function foo to bar"), assert the proposed diff matches expectations, approve all, assert the buffer content is correct, assert the sidecar was updated.
- [ ] 14.2. Since this depends on Ollama: gate behind a feature flag `e2e-ai-tests`. CI runs it only on request.
- [ ] 14.3. Commit: `test(chat): end-to-end agent loop integration test`.

### 15. Quality gates + documentation

- [ ] 15.1. Standard gates.
- [ ] 15.2. Write `/docs/CHAT_PANEL.md` describing every piece of the panel and the loop.
- [ ] 15.3. Write `/docs/AGENT_FLOW.md` — the user-facing guide: "how to give a task to an agent, review, approve."
- [ ] 15.4. Tag: `git tag -a m23-complete -m "M23 complete: integrated AI chat panel"`; push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. `Ctrl+L` toggles the chat panel.
3. Submitting a prompt against a configured provider produces a streaming response.
4. Tool calls execute correctly.
5. Edit proposals render as inline diffs and can be accepted/rejected per hunk.
6. Accepted edits are undo-able in one step.
7. Sidecars update after commit.
8. Token counter reflects usage.
9. Cancel works at any stage.
10. `m23-complete` tag pushed.

## Testing Requirements

- Unit: agent loop state machine with mocked provider.
- Unit: conversation persistence round-trip.
- Integration: end-to-end flow with Ollama.
- Manual: use it for real work for a day.

## Git Commit Strategy

13-16 commits. Push after items 3, 7, 9, 13, 15.

## Handoff to M24

M24 is release — tag `0.3.0-v3`, run installers, ship.

---

## Standing Orders Reminder

- Tool execution during streaming must never block the main thread. Route through `WorkerPool`.
- Cancellation must actually cancel — not just hide UI. Drop the provider stream, drop the tx.
- Agent loops with no hard iteration cap become infinite loops. The 25-round cap is not optional.
- Every proposed edit must be reviewable before commit. Do not add an "auto-accept" mode in V3 — it belongs in V4+ with proper guardrails.

Go.
