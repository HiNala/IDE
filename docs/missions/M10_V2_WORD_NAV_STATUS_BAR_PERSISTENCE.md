# M10 — V2: Word Nav, Status Bar, Persistence, Final Acceptance

**Mission ID:** M10
**Prerequisites:** M09 complete. Gutter, selection, clipboard, mouse, shift-arrow all working.
**Output:** The remaining V2 PRD items are shipped. A minimal status bar renders at the bottom of the window showing file path, line/column, modification state, and encoding/line-ending. Word-level navigation shortcuts are universally consistent. The editor remembers the last-opened file and window size across launches via a small persisted config file. A written V2 acceptance report confirms every V2 PRD line item. A `0.2.0-v2` release candidate is tagged.
**Estimated scope:** 1-2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/reference/05_V2_PRD.md` — the full V2 spec. §4 (features), §5 (UI philosophy), §6 (interaction model), §11 (acceptance).
- `/docs/ARCHITECTURE.md` — `editor-ui`'s role.
- `/docs/CROSS_PLATFORM.md` — XDG paths on Linux, AppData on Windows, Application Support on macOS.

---

## The Situation In Plain English

After M09 the editor is usable. M10 finishes the V2 PRD by adding the small set of remaining affordances that separate a usable editor from a pleasant one: a status bar so the user always knows where they are; word-level navigation to match what every other editor gives you; and the "reopen last file" persistence so launching the app feels continuous with the last session. None of these are deep engineering — but each one meaningfully improves the experience, and shipping them all in one disciplined mission closes out the V2 scope cleanly.

After implementing, we run the same kind of acceptance pass we did in M08 for the MVP, but targeted at the V2 PRD's criteria. The result is a second acceptance report (`/docs/V2_ACCEPTANCE.md`) and a second release tag (`0.2.0-v2`). At that point the editor is a legitimate daily-driver for "edit files" tasks, and the next expansion (syntax highlighting, LSP, AI, plugins) can begin with confidence.

---

## Scope

**In scope:**
- Word-level cursor navigation: Ctrl+Left / Ctrl+Right move by word; Shift-Ctrl variants extend selection; Ctrl+Backspace / Ctrl+Delete word-delete. These were largely implemented in M05; this mission ensures they are universally consistent and documented.
- Status bar at the bottom: file name, dirty indicator, cursor line/col, total line count, encoding (`UTF-8` / `UTF-16 LE` / `UTF-8 BOM`), line ending (`LF` / `CRLF` / `CR`).
- Persisted config: `$CONFIG_DIR/ide/state.json` stores last-opened file path, window size, window position, scroll offset, and cursor position at exit. Restored on next launch.
- V2 acceptance report.
- Final documentation sweep.

**Out of scope:**
- Theme customization (post-V2).
- Customizable keybindings (post-V2).
- Per-file cursor/scroll memory beyond just the last file (post-V2).
- Recent-files list (post-V2).
- Preferences UI (post-V2; config is hand-edited JSON for V2).

---

## North Star

Quitting the editor with a file open, cursor at line 2345, scroll halfway down, and re-launching via `editor-app` (no CLI args) opens the editor in exactly the same state: same file, same cursor, same scroll, same window size and position. The status bar reads "*path/to/file.rs · Ln 2345 · Col 12 · UTF-8 · LF" with the `*` appearing only when the buffer is dirty.

---

## TODO List

### 1. Finalize word navigation

- [ ] 1.1. Audit every keybinding that takes a `CursorMotion`. Ensure `WordLeft` / `WordRight` are available as: `Ctrl+Left`, `Ctrl+Right`, `Shift+Ctrl+Left`, `Shift+Ctrl+Right` (extend selection). On macOS: map `Alt+Left` / `Alt+Right` to the same thing (Apple's convention).
- [ ] 1.2. Ensure `Ctrl+Backspace` = `DeleteWordBackward`, `Ctrl+Delete` = `DeleteWordForward`. On macOS: `Alt+Backspace` / `Alt+Delete`.
- [ ] 1.3. Verify behavior on mixed CJK + Latin content: the word-boundary logic should treat CJK runs as "one word" (or each character as a word — match VS Code's behavior: each CJK char is its own "word" for navigation purposes).
- [ ] 1.4. Unit tests covering: word-by-word through ASCII, Unicode punctuation, CJK, emoji, mixed.
- [ ] 1.5. Commit: `feat(input, core): finalize word-level navigation and deletion shortcuts`.

### 2. Design and build the `StatusBar` layer

- [ ] 2.1. `crates/editor-ui/src/status_bar.rs`:
  ```rust
  pub struct StatusBar {
      height: f32,        // logical pixels
      font_size: f32,
      text_areas: Vec<TextArea>,
  }
  impl StatusBar {
      pub fn new(font_size: f32) -> Self;
      pub fn height(&self) -> f32;
      pub fn prepare(&mut self, info: StatusBarInfo, viewport: PhysicalSize<u32>, scale: f32) -> Result<(), RenderError>;
  }
  pub struct StatusBarInfo {
      pub path: Option<PathBuf>,
      pub dirty: bool,
      pub cursor_line: usize,
      pub cursor_col: usize,
      pub total_lines: usize,
      pub encoding: Encoding,
      pub line_ending: LineEnding,
      pub external_modified: bool,
  }
  ```
- [ ] 2.2. Render a 24-logical-pixel tall bar at the bottom. Background: a single dark quad (via `QuadLayer`) slightly lighter than the main background. Text drawn by `TextLayer` via queued `TextArea`s.
- [ ] 2.3. Layout: left side = file path (truncated with `…` if too long), with a `*` prefix if dirty and a `⚠` if externally modified. Middle-left = "Ln N, Col M · total L lines". Right side = encoding + line ending.
- [ ] 2.4. Body text area reduced by the status bar height: `TextLayer::prepare` needs to clip to `viewport_height - status_height` when the status bar is active. Pass this as an additional parameter.
- [ ] 2.5. Unit test: `StatusBar::prepare` with various `StatusBarInfo` produces expected number of TextAreas.
- [ ] 2.6. Commit: `feat(ui): add StatusBar with file/cursor/encoding/line-ending info`.

### 3. Wire `StatusBar` into `Renderer`

- [ ] 3.1. `Renderer` gains a `status_bar: StatusBar` field.
- [ ] 3.2. `FrameInput` gains a `status: StatusBarInfo` field.
- [ ] 3.3. Render order: `selection_layer.prepare` → `status_bar.prepare` → `text_layer.prepare(..., viewport_height - status_bar.height())` → acquire → pass → selection render → text render → status-bar-bg quad → cursor quad → present.
- [ ] 3.4. `EditorState` grows a `status_info(&self) -> StatusBarInfo` method that packages current state for the host.
- [ ] 3.5. Commit: `refactor(render, app): route StatusBarInfo through FrameInput`.

### 4. Implement persisted config

- [ ] 4.1. Add `dirs` crate (for cross-platform config dir lookup: `ProjectDirs::from("com", "HiNala", "IDE")`). This gives us the right dir on Windows (`%APPDATA%\HiNala\IDE`), macOS (`~/Library/Application Support/com.HiNala.IDE`), Linux (`~/.config/ide`).
- [ ] 4.2. `crates/editor-app/src/config.rs`:
  ```rust
  #[derive(Serialize, Deserialize, Default)]
  pub struct PersistedState {
      pub last_file: Option<PathBuf>,
      pub last_cursor_byte: Option<u64>,
      pub last_scroll_y: Option<f32>,
      pub window_size: Option<(u32, u32)>,
      pub window_pos: Option<(i32, i32)>,
      pub version: u32,    // for future schema migrations
  }
  impl PersistedState {
      pub fn load() -> Self;    // returns default on any error (first launch, corrupted file, etc.)
      pub fn save(&self) -> std::io::Result<()>;
  }
  ```
- [ ] 4.3. Add `serde` + `serde_json` as deps in `editor-app`.
- [ ] 4.4. `save` writes JSON to `$config_dir/ide/state.json` using the same atomic-write pattern as `editor-io::save_file_sync` (write temp, fsync, rename). Never corrupt the user's state file.
- [ ] 4.5. `load` reads the same file. On any error (missing, parse failure, wrong version), logs at `info!` and returns `Default::default()`. No panics.
- [ ] 4.6. Call `load` during `resumed` before creating the window — use the size if present. Call `save` during `exiting` handler (winit 0.30's `ApplicationHandler::exiting`).
- [ ] 4.7. Integration test: write a fake state file, load it, verify fields round-trip. Corrupt it; verify graceful fallback.
- [ ] 4.8. Commit: `feat(app): persist last-file, cursor, scroll, and window geometry across launches`.

### 5. Restore state on launch

- [ ] 5.1. During `resumed`: check `PersistedState::load()`. If `last_file` is present and exists on disk, kick off a load for it; after load, restore `cursor` to `last_cursor_byte` and `scroll` to `last_scroll_y`.
- [ ] 5.2. If a CLI arg is also provided, the CLI arg wins (opens the specified file; persisted state is reset to reflect the new file at next save).
- [ ] 5.3. If the persisted file no longer exists, log `info!` and open an empty buffer.
- [ ] 5.4. Window creation: use `WindowAttributes::with_inner_size` and `.with_position` from the persisted values when present; otherwise default to 1280×720 centered.
- [ ] 5.5. Save state on: `exiting`, every successful file save, and every 60 seconds if dirty (handle via a timer-driven worker job or just a counter in the frame loop).
- [ ] 5.6. Commit: `feat(app): restore persisted state on launch`.

### 6. Polish: clamp cursor and scroll to valid ranges on restore

- [ ] 6.1. If the persisted file changed externally since last close, the cursor byte offset might be out of range. On restore, clamp: `min(last_cursor_byte, buffer.len_bytes())`. Same for scroll: `min(last_scroll_y, max_scroll)`.
- [ ] 6.2. If the saved window position is off-screen (monitor unplugged), winit clamps automatically, but double-check by reading `Window::inner_position` after creation and adjusting if needed.
- [ ] 6.3. Commit: `fix(app): clamp restored cursor/scroll/window to valid ranges`.

### 7. Add a tiny "About" / "Help" affordance

- [ ] 7.1. `F1` or `?` key (with no modifier, after Esc) opens a minimal help overlay listing keybindings. Render it as a translucent card over the buffer via `QuadLayer` + `TextLayer`. Press `F1` / `Esc` again to dismiss.
- [ ] 7.2. The help card lists: navigation, editing, selection, clipboard, save/load, undo/redo, dev overlay. One line per shortcut.
- [ ] 7.3. Not strictly a V2 PRD item but it's so small and so useful that it's included here as polish.
- [ ] 7.4. Commit: `feat(ui): add F1 help overlay`.

### 8. Prepare the V2 acceptance report

- [ ] 8.1. Create `/docs/V2_ACCEPTANCE.md` with the same table structure as `/docs/MVP_ACCEPTANCE.md`. Rows come from `/reference/05_V2_PRD.md` §11 and §12.
- [ ] 8.2. Additionally carry forward every MVP acceptance row — V2 must not regress MVP.
- [ ] 8.3. Commit: `docs(acceptance): draft V2 acceptance checklist`.

### 9. Run the V2 acceptance tests

- [ ] 9.1. Go through each row. Record measured numbers and observations.
- [ ] 9.2. Confirm all five V2 PRD feature items (line numbers, selection, clipboard, file path display, undo/redo) are ✅.
- [ ] 9.3. Confirm interaction-model items (shift-select, word nav, auto-scroll on drag, smooth scroll) are ✅.
- [ ] 9.4. Confirm performance preservation: p50/p95/p99 frame time within 5% of MVP `m08-mvp` baseline. If worse, investigate.
- [ ] 9.5. Cross-platform pass on all three OSes.
- [ ] 9.6. Commit: `docs(acceptance): fill V2 acceptance report`.

### 10. Benchmarks

- [ ] 10.1. Benchmark status bar `prepare` cost: should be negligible (<50 μs).
- [ ] 10.2. Benchmark `PersistedState::save` atomic-write path.
- [ ] 10.3. Save baseline as `m10-v2`.
- [ ] 10.4. Commit: `bench(ui, app): status bar and persisted-state overhead`.

### 11. Quality gates + release

- [ ] 11.1. All standard gates.
- [ ] 11.2. Release build on all three OSes; smoke test.
- [ ] 11.3. Bump workspace version to `0.2.0`.
- [ ] 11.4. Update `CHANGELOG.md` with the V2 section.
- [ ] 11.5. Tag `0.2.0-v2`: `git tag -a 0.2.0-v2 -m "V2 release: minimal useful editor"`; push.
- [ ] 11.6. Also tag `m10-complete`.

### 12. Documentation

- [ ] 12.1. Update `/docs/STATUS.md`: M10 complete, M11 next (release engineering).
- [ ] 12.2. Update `/docs/ARCHITECTURE.md` with the status bar and persistence additions.
- [ ] 12.3. Update `/docs/CROSS_PLATFORM.md` with the config dir locations on each OS.
- [ ] 12.4. Update README: the editor is now "a minimal usable editor," not just "an MVP proving performance."
- [ ] 12.5. Commit: `docs: V2 wrap-up and README update`.

---

## Validation / Acceptance Criteria

M10 is complete when:

1. Quality gates pass.
2. Every row in `/docs/V2_ACCEPTANCE.md` is green (or yellow with a documented follow-up).
3. All previously-green MVP acceptance rows remain green.
4. Word navigation works on ASCII, Unicode, and mixed content.
5. Status bar renders accurate info and updates live as the cursor and buffer change.
6. Persisted state survives a full quit-and-relaunch cycle.
7. `F1` help overlay works.
8. `0.2.0-v2` tag pushed.
9. Performance within 5% of `m08-mvp` baseline.

## Testing Requirements

- Unit tests for word navigation edge cases.
- Integration test for persisted state round-trip.
- Manual cross-platform acceptance.

## Git Commit Strategy

10-14 commits. Push after items 2, 4, 5, 7, 9, 11.

## Handoff to M11

M11 assumes:

- `0.2.0-v2` is the canonical "release-ready" version.
- M11 takes this binary and produces distributable installers for each OS, handles code signing / notarization where applicable, and publishes to GitHub Releases.

---

## Standing Orders Reminder

- Persisted state is the next-most-fragile thing after file I/O. Treat it with the same atomic-write discipline. A corrupted `state.json` should never crash the app — just silently fall back to defaults.
- Keep the status bar simple. No icons, no animations, no context menus, no per-token hover cards. Those are post-V2.
- The F1 help overlay must be scannable in 30 seconds. If it grows past one screen, something is wrong.

Go.
