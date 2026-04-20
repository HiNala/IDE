# M05 — Frame Loop, Input Pipeline, Performance Budgets

**Mission ID:** M05
**Prerequisites:** M04 complete. Text renders. Ad-hoc input in `editor-app` works for scrolling and arrow keys.
**Output:** A real `editor-input` crate that owns the full input pipeline (keyboard, IME, mouse lifecycle). A formal frame loop in `editor-app` that enforces per-subsystem performance budgets. Typing characters into the editor works. IME composition works for CJK and dead-key sequences. Input-to-pixel latency is under 5 ms under load.
**Estimated scope:** 2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/docs/INPUT_AND_IME.md` — our winit 0.30 input model, IME handling, key mapping.
- `/docs/ARCHITECTURE.md` — frame loop, direct-to-state pipeline.
- `/docs/PERFORMANCE_BUDGETS.md` — per-subsystem time budgets.
- `/reference/02_ARCHITECTURE_STRATEGY.md` §8 — the direct-to-state input architecture from the PRD.
- `/reference/03_GAPS_AND_RISKS.md` §4 — IME and international input considerations.
- `https://docs.rs/winit/latest/winit/event/enum.WindowEvent.html` — canonical event reference.
- `https://docs.rs/winit/latest/winit/event/enum.Ime.html` — IME semantics.

---

## The Situation In Plain English

The editor currently has a scaffolded input path inside `editor-app`'s `ApplicationHandler`. That path handles a few arrow keys and exits. M05 replaces that with a disciplined, production-grade input pipeline that lives in its own crate, `editor-input`, and pipes keyboard + IME events straight to the text engine without any intermediate queue, framework, or async hop. The reason this matters is latency: every layer between keypress and pixel adds microseconds. Our target is under 5 milliseconds end-to-end from keystroke to screen under normal load, and the only way we hit that on a 10 MB file is to keep the path short, deterministic, and on the main thread.

The input pipeline is a function: `(winit::WindowEvent, &mut EditorState) -> Option<EditorCommand>`. It has no internal mutable state except a small IME composition buffer. It runs synchronously inside `ApplicationHandler::window_event`. It produces a command (insert text, delete, move cursor, scroll, save, quit), which the host immediately applies to the `EditorState` and then requests a redraw. The entire round-trip happens inside one winit event dispatch.

Alongside the input pipeline, M05 formalizes the frame loop. We captured frame timing in M03 but did not yet enforce a budget. Now we do: each frame logs a warning if the prepare phase exceeds 4 ms, the GPU submission exceeds 6 ms, or the total frame time exceeds 16 ms (at 60 Hz) or 8 ms (at 120 Hz, when `PresentMode::Immediate` is active). These warnings are our early-warning system for regressions. They are silenced in release builds by default but enabled with `RUST_LOG=editor_app::frame=warn`.

Finally, M05 introduces the *worker thread pool* foreshadowed in the architecture docs. We do not actually use it yet — file I/O (M06) is the first real customer. But we create it, test it, and make it available so M06 has somewhere to put blocking work. The pool is small (`num_cpus - 1`, minimum 1, maximum 8), uses `std::thread` and `std::sync::mpsc` channels (no tokio), and supports cancellation via a shared `AtomicBool` token.

---

## Scope

**In scope:**
- New `editor-input` crate content: `EventTranslator`, `KeyMap`, `ImeState`, `EditorCommand` enum.
- Keyboard → command mapping for all edit operations: insert char, insert newline, backspace, delete, tab, navigation (arrows, home/end, pageup/pagedown, ctrl+home/ctrl+end, ctrl+left/right for word moves — word-move logic itself lives in `editor-core` and is exposed here), undo (Ctrl+Z), redo (Ctrl+Y / Ctrl+Shift+Z).
- IME preedit + commit handling.
- Dead-key composition on platforms where winit delivers it as IME events.
- Modifier tracking (Ctrl, Shift, Alt, Meta/Cmd/Super).
- Platform-aware shortcut modifier (Cmd on macOS, Ctrl elsewhere).
- `EditorState` struct in `editor-app` that owns `TextBuffer`, `Cursor`, `Selection`, `UndoStack`, `ScrollOffset`.
- Frame loop with per-phase timing and budget warnings.
- Worker thread pool in a new `editor-core::worker` module (scoped to `editor-core` because it's used by I/O, editor internals, future parsing — not just the app).

**Out of scope:**
- Mouse selection and click-to-position (M09).
- Clipboard (M09).
- Multi-cursor (post-V2).
- File save triggered by Ctrl+S (M06).
- Find/replace (post-V2).
- Any text rendering changes.

---

## North Star

At the end of M05, the editor is *usable* for typing. A developer can:

- Open it.
- Type text, including typing Chinese / Japanese / Korean via IME, or French/German/Spanish via dead-key sequences.
- Navigate with all the usual editing shortcuts.
- Undo and redo.
- See a frame-timing log line every few seconds showing p50/p95/p99 frame time.
- Feel no perceptible lag between keystroke and pixel.

---

## TODO List

### 1. Design the `EditorCommand` enum

- [ ] 1.1. `crates/editor-input/src/command.rs`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum EditorCommand {
      InsertText(String),           // One or more chars (IME commit can deliver multi-char)
      InsertNewline,
      DeleteBackward,               // Backspace
      DeleteForward,                // Delete
      DeleteWordBackward,           // Ctrl+Backspace
      DeleteWordForward,            // Ctrl+Delete
      MoveCursor(CursorMotion),
      ExtendSelection(CursorMotion),
      Scroll(ScrollDelta),
      Undo,
      Redo,
      Save,                         // Ctrl+S — handled in M06
      Quit,                         // Esc (debug) or Ctrl+Q / Alt+F4
      SetImePreedit { text: String, cursor: Option<(usize, usize)> },
      ClearImePreedit,
      None,                         // Nothing happened (filtered event)
  }
  ```
- [ ] 1.2. `ScrollDelta` captures lines / pages / pixels. Keep it an enum with three variants: `Lines(i32)`, `Pages(i32)`, `Pixels(f32)`.
- [ ] 1.3. Commit: `feat(input): define EditorCommand enum`.

### 2. Implement `KeyMap`

- [ ] 2.1. `crates/editor-input/src/keymap.rs`. The keymap is a function `map(&KeyEvent, &Modifiers, &PlatformMod) -> Option<EditorCommand>`. Static for MVP — no user customization yet.
- [ ] 2.2. Decide the modifier abstraction: `PlatformMod` returns `Ctrl` on Windows/Linux and `Cmd` on macOS. Wrap it so the same `Ctrl+C` in our keymap means `Cmd+C` on macOS.
- [ ] 2.3. Handle `KeyEvent::logical_key` — the cooked, layout-aware key. Prefer this for character input. Reserve `KeyEvent::physical_key` for shortcut handling (so that Ctrl+Z always undoes even on Dvorak layouts).
- [ ] 2.4. Handle `KeyEvent::text` for inserting plain characters. It's `Option<SmolStr>` and is `None` for control keys. When present, emit `EditorCommand::InsertText(text.to_string())`.
- [ ] 2.5. Respect `KeyEvent::repeat` — we pass repeats through for typing and navigation, but not for Ctrl+Z / Ctrl+S (mash Ctrl+Z should still only trigger once per key-down, even if winit delivers repeats — actually, the convention is to respect repeats for undo too; document the choice).
- [ ] 2.6. Implement every shortcut from Scope:
  - Character typing via `text` when no control modifier is held.
  - `Enter` → `InsertNewline`.
  - `Backspace` → `DeleteBackward` (or `DeleteWordBackward` with Ctrl).
  - `Delete` → `DeleteForward` (or `DeleteWordForward` with Ctrl).
  - `Tab` → `InsertText("    ")` for MVP (real tab-vs-spaces logic is V2+; document the hard-coded 4 spaces).
  - Arrows: `Left`/`Right` → `MoveCursor(Left|Right)`; `Up`/`Down` → `MoveCursor(Up|Down)`; with Shift held → `ExtendSelection(...)`.
  - `Ctrl+Left`/`Ctrl+Right` → word moves.
  - `Home`/`End` → line start/end.
  - `Ctrl+Home`/`Ctrl+End` → buffer start/end.
  - `PageUp`/`PageDown` → scroll + move cursor.
  - `Ctrl+Z` → `Undo`. `Ctrl+Y` / `Ctrl+Shift+Z` → `Redo`.
  - `Ctrl+S` → `Save`.
  - `Escape` → `Quit` (debug only; remove when menus ship).
- [ ] 2.7. Unit tests: table-driven test mapping every expected `(KeyEvent, Modifiers)` tuple to its expected `EditorCommand`.
- [ ] 2.8. Commit: `feat(input): implement KeyMap with platform-aware modifier`.

### 3. Add word-move logic to `editor-core`

- [ ] 3.1. In `editor-core::cursor`, extend `CursorMotion` with `WordLeft` and `WordRight` variants.
- [ ] 3.2. Implement word boundaries with `unicode-segmentation`'s `UnicodeWordIndices` iterator, or a simpler approach: step past whitespace then step past non-whitespace (the classic editor "word" definition; matches VS Code).
- [ ] 3.3. Unit tests covering: move across punctuation, across whitespace runs, at buffer boundaries, on CJK text (where "words" are a fuzzy concept — our implementation should at least not crash or hang).
- [ ] 3.4. Commit: `feat(core): add word-boundary cursor motion`.

### 4. Implement `ImeState`

- [ ] 4.1. `crates/editor-input/src/ime.rs`:
  ```rust
  pub struct ImeState {
      composing: Option<Composing>,
  }
  struct Composing {
      preedit: String,
      cursor: Option<(usize, usize)>, // byte range within `preedit`
  }
  ```
- [ ] 4.2. Handle `WindowEvent::Ime(Ime::Enabled)` → reset state.
- [ ] 4.3. Handle `WindowEvent::Ime(Ime::Preedit(text, cursor))` → produce `EditorCommand::SetImePreedit { text, cursor }`. If `text` is empty, `ClearImePreedit`.
- [ ] 4.4. Handle `WindowEvent::Ime(Ime::Commit(text))` → produce `EditorCommand::InsertText(text)` and also `ClearImePreedit`.
- [ ] 4.5. Handle `WindowEvent::Ime(Ime::Disabled)` → `ClearImePreedit`.
- [ ] 4.6. Ensure the preedit text is *displayed*. Two options:
  - (a) Insert preedit into the rope at the cursor position and remove it on every preedit update. Simple; version bumps a lot.
  - (b) Store preedit separately in `EditorState` and render it as an overlay in `TextLayer`. Complex but cleaner.
  - For MVP, pick (a): easier to implement, correct behavior, only pain is some extra rope churn during IME sessions (which are short).
- [ ] 4.7. `Window::set_ime_allowed(true)` must be called on window creation. Also set the IME cursor area so the candidate window pops up near the text cursor: on preedit update, compute the cursor's screen rect and call `Window::set_ime_cursor_area(Position, Size)`.
- [ ] 4.8. Unit tests that simulate Japanese hiragana→kanji composition: Preedit("a", (1,1)), Preedit("あ", (1,1)), Preedit("あb", (2,2)), Preedit("編集", (2,2)), Commit("編集"). Assert commands emitted.
- [ ] 4.9. Commit: `feat(input): implement IME preedit + commit with cursor-area reporting`.

### 5. Implement `EventTranslator`

- [ ] 5.1. The crate's public entry point. `crates/editor-input/src/lib.rs`:
  ```rust
  pub struct EventTranslator {
      keymap: KeyMap,
      ime: ImeState,
      modifiers: Modifiers,
  }
  impl EventTranslator {
      pub fn new() -> Self;
      pub fn translate(&mut self, event: &WindowEvent) -> EditorCommand;
  }
  ```
- [ ] 5.2. `translate` updates `modifiers` on `ModifiersChanged`, delegates to `keymap` on `KeyboardInput`, delegates to `ime` on `Ime`, returns `EditorCommand::None` for other events.
- [ ] 5.3. Rustdoc + doctest showing a minimal usage example.
- [ ] 5.4. Commit: `feat(input): implement EventTranslator top-level API`.

### 6. Create `EditorState` in `editor-app`

- [ ] 6.1. `crates/editor-app/src/state.rs`:
  ```rust
  pub struct EditorState {
      pub buffer: TextBuffer,
      pub cursor: Cursor,
      pub selection: Selection,
      pub undo: UndoStack,
      pub scroll: ScrollOffset,
      pub ime_preedit: Option<(String, Option<(usize, usize)>)>,
  }
  impl EditorState {
      pub fn apply(&mut self, cmd: EditorCommand) -> CommandResult;
  }
  ```
- [ ] 6.2. `apply` is the direct-to-state mutation. For each command:
  - `InsertText(s)` / `InsertNewline` → `buffer.apply_edit(Insert { pos: cursor, text })`, push to undo, advance cursor, checkpoint-on-non-insert-boundary rules, mark dirty.
  - `DeleteBackward` → if selection non-empty delete selection; else delete previous grapheme. Push inverse to undo.
  - `MoveCursor(m)` → `cursor.apply(m, &buffer)`; collapse selection to cursor.
  - `ExtendSelection(m)` → move cursor; adjust `selection.head`.
  - `Scroll(delta)` → adjust `scroll.y_px` and clamp.
  - `Undo`/`Redo` → delegate to `UndoStack`.
  - `Save` → delegate to `editor-io::save(...)` in M06 (for now, log a warning that save is not yet implemented).
  - `Quit` → return `CommandResult::Quit`.
  - `SetImePreedit` / `ClearImePreedit` → mutate the preedit buffer.
- [ ] 6.3. `CommandResult` tells the host whether to redraw, quit, or both. Normal commands → `Redraw`. Quit → `Quit`. Empty selections moving → `Redraw` (cursor position changed).
- [ ] 6.4. Commit: `feat(app): introduce EditorState and command application`.

### 7. Wire `EventTranslator` + `EditorState` into `ApplicationHandler`

- [ ] 7.1. `App` gains fields `translator: EventTranslator`, `state: EditorState`.
- [ ] 7.2. In `window_event`:
  ```rust
  let cmd = self.translator.translate(&event);
  match self.state.apply(cmd) {
      CommandResult::Quit => event_loop.exit(),
      CommandResult::Redraw => self.window.as_ref().unwrap().request_redraw(),
      CommandResult::None => {}
  }
  ```
- [ ] 7.3. Still handle `Resized`, `ScaleFactorChanged`, `CloseRequested`, `RedrawRequested` directly for the render path.
- [ ] 7.4. Call `Window::set_ime_allowed(true)` once during `resumed`.
- [ ] 7.5. On cursor movement, compute the cursor's screen rect and call `Window::set_ime_cursor_area`.
- [ ] 7.6. Commit: `feat(app): wire EventTranslator and EditorState into winit loop`.

### 8. Implement the frame loop with budget tracking

- [ ] 8.1. `crates/editor-app/src/frame.rs` (or inline in `main.rs`). Structure every redraw as:
  ```rust
  let t0 = Instant::now();
  // Phase 1: prepare (CPU-heavy: layout cache, text shaping)
  self.renderer.text_layer.prepare(&input)?;
  let t1 = Instant::now();
  // Phase 2: GPU submission
  let frame = self.renderer.acquire()?;
  self.renderer.render_frame_into(frame, &input)?;
  let t2 = Instant::now();
  // Record metrics
  let prepare = t1 - t0;
  let submit  = t2 - t1;
  let total   = t2 - t0;
  self.frame_timer.record(prepare, submit, total);
  ```
- [ ] 8.2. Budget warnings:
  - `prepare > 4ms` → `warn!("prepare phase exceeded budget: {:?}", prepare)`.
  - `submit > 6ms` → similar.
  - `total > 16ms` (at 60 Hz) or `> 8ms` (at 120 Hz, detected from present mode) → similar.
  Suppress in release builds unless `RUST_LOG=editor_app::frame=warn`.
- [ ] 8.3. Track p50/p95/p99 over the last 120 frames; log a summary every 2 seconds at `debug!`.
- [ ] 8.4. Commit: `feat(app): formalize frame loop with per-phase budget tracking`.

### 9. Implement the worker thread pool

- [ ] 9.1. `crates/editor-core/src/worker.rs`. Tiny API:
  ```rust
  pub struct WorkerPool { /* ... */ }
  pub struct JobToken { cancelled: Arc<AtomicBool> }

  impl WorkerPool {
      pub fn new(n_threads: Option<usize>) -> Self;
      pub fn spawn<F, T>(&self, job: F) -> (JobToken, Receiver<T>)
      where F: FnOnce(&JobToken) -> T + Send + 'static, T: Send + 'static;
      pub fn thread_count(&self) -> usize;
  }
  impl JobToken {
      pub fn cancel(&self);
      pub fn is_cancelled(&self) -> bool;
  }
  ```
- [ ] 9.2. Implementation: fixed-size `Vec<JoinHandle>` of worker threads pulling jobs from a bounded MPMC channel. Use `crossbeam_channel` for the MPMC if we don't already depend on it; otherwise `std::sync::mpsc` works for MPSC with a trick, but bounded MPMC is simpler with crossbeam. Add `crossbeam-channel = "0.5"` to `editor-core`.
- [ ] 9.3. Thread count: caller can pass `None` to get `num_cpus::get().saturating_sub(1).clamp(1, 8)`. Add `num_cpus = "1"` as a dep.
- [ ] 9.4. `spawn` returns a `(JobToken, Receiver<T>)` so the caller can poll the result or cancel. Cancellation is cooperative — the job must check `token.is_cancelled()` periodically. Document this.
- [ ] 9.5. `Drop` on `WorkerPool` sends shutdown signal and joins all threads cleanly.
- [ ] 9.6. Unit tests: submit 100 jobs, assert all complete; submit and cancel, assert the job sees cancellation; drop the pool mid-job and verify clean shutdown.
- [ ] 9.7. Commit: `feat(core): add WorkerPool for background jobs with cancellation`.

### 10. Wire a `WorkerPool` instance into `editor-app`

- [ ] 10.1. `EditorState` gains a `worker_pool: Arc<WorkerPool>` field (or `App` owns it directly; `EditorState` borrows). Pick one based on whose lifetime matches.
- [ ] 10.2. Nothing uses it yet. Add a placeholder `#[allow(dead_code)]` if clippy complains; remove the allow in M06 when file I/O uses it.
- [ ] 10.3. Commit: `feat(app): instantiate WorkerPool in application startup`.

### 11. Latency measurement

- [ ] 11.1. Add a debug feature `latency-trace`: when enabled, the keymap records `Instant::now()` at the moment it emits a command, and the frame loop records the timestamp when the resulting render completes. Log the delta.
- [ ] 11.2. Expected numbers on Windows on a modern CPU + discrete GPU: 2-4 ms for a single-char insert. Document baselines in `/docs/PERFORMANCE_BUDGETS.md`.
- [ ] 11.3. Commit: `feat(app): add latency-trace feature for end-to-end input→pixel timing`.

### 12. Edge cases

- [ ] 12.1. Typing while scrolled to the middle of the doc: cursor should stay where we are, scroll should auto-follow if we hit the bottom.
- [ ] 12.2. Typing a surrogate-pair emoji or a 4-byte UTF-8 char: confirm it goes into the buffer correctly and renders as one grapheme.
- [ ] 12.3. Key repeat during rapid typing: no dropped inputs, no ordering inversions.
- [ ] 12.4. Undo during an IME composition: clear the preedit first, then apply undo to the pre-composition state. Test the sequence.
- [ ] 12.5. Paste a multi-line string via simulated `InsertText("line1\nline2\nline3")` — confirm line count updates, cursor lands on line 3 col 5, no rope corruption.
- [ ] 12.6. A very long continuous typing burst (1000+ chars/sec): frame loop keeps up, no input loss.
- [ ] 12.7. Commit: `fix(input): handle rapid typing, undo-during-IME, multiline insert, emoji`.

### 13. Benchmarks

- [ ] 13.1. `crates/editor-input/benches/translate.rs`: measure `EventTranslator::translate` throughput. Should be trivially fast (sub-microsecond).
- [ ] 13.2. `crates/editor-app/benches/end_to_end.rs` (may require custom harness): measure a full `translate → apply → render_frame` cycle for a single-char insert. Target p99 < 5 ms on reference hardware.
- [ ] 13.3. Save baseline as `m05-mvp`.
- [ ] 13.4. Commit: `bench(input, app): measure input translation and end-to-end frame cost`.

### 14. Quality gates

- [ ] 14.1. `cargo fmt --all --check`.
- [ ] 14.2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] 14.3. `cargo test --workspace`.
- [ ] 14.4. `cargo bench -p editor-input -p editor-app --no-run`.
- [ ] 14.5. Manual: type a paragraph of Lorem Ipsum in the editor, Undo/Redo it, type some Japanese via IME, move the cursor around with every shortcut in scope.

### 15. Documentation

- [ ] 15.1. Update `/docs/INPUT_AND_IME.md` with final semantics and the list of keybindings implemented.
- [ ] 15.2. Update `/docs/ARCHITECTURE.md` with the frame-loop phase diagram and the worker-pool description.
- [ ] 15.3. Update `/docs/PERFORMANCE_BUDGETS.md` with the observed numbers on reference hardware.
- [ ] 15.4. Update `/docs/STATUS.md`: M05 complete, M06 next.
- [ ] 15.5. Update `/CHANGELOG.md`.
- [ ] 15.6. Tag: `git tag -a m05-complete -m "M05 complete: input pipeline and frame loop"`; push.

---

## Validation / Acceptance Criteria

M05 is complete when:

1. Quality gates pass.
2. CI green on all three OSes.
3. Typing into the editor works on Windows. Every shortcut in Scope produces the expected behavior.
4. IME composition works for Japanese (or another CJK language if you can't install a Japanese IME easily).
5. Dead-key composition works for French accented vowels.
6. Undo/redo works correctly for typing sessions with coalescing.
7. Frame loop logs p50/p95/p99 every 2 s.
8. End-to-end input-to-pixel p99 < 5 ms on a 10 MB file (captured via `latency-trace`).
9. `WorkerPool` has unit tests and is available to downstream crates.
10. Benchmarks saved as `m05-mvp`.
11. `m05-complete` tag pushed.

## Testing Requirements

- Unit tests for keymap (table-driven), IME state, command application.
- Property tests: random command sequences preserve buffer/undo invariants.
- Benchmarks captured.
- Manual cross-platform verification.

## Git Commit Strategy

12-16 commits. Push after items 2, 4, 7, 8, 9, 12, 15.

## Handoff to M06

M06 assumes:

- `EditorCommand::Save` exists but currently logs a warning. M06 wires it to real file I/O.
- `WorkerPool` is available in `editor-core`. M06 uses it for async file loads.
- `EditorState` is the mutation surface. M06 adds `open_file`/`save_file` methods to it.

---

## Standing Orders Reminder

- The input pipeline must stay *synchronous* on the main thread. Do not introduce an async queue, a channel, or a worker hop between keypress and state mutation. That would add latency. Only long-running operations (file I/O, future parsing) go on the worker pool.
- Do not debounce key events. Every keystroke matters.
- If the budget warnings fire on your own machine during normal editing, *fix the regression* before merging. Do not adjust the budget upward.
- Preserve determinism: the same input sequence applied to the same starting state must produce the same ending state. This is the foundation for future collaborative editing and reliable debugging.

Go.
