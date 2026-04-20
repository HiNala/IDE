# M14 — Sidebar, Tab Strip, Quick Open

**Mission ID:** M14
**Prerequisites:** M13 complete. `Workspace` and `BufferManager` exist and are tested.
**Output:** Three new UI surfaces that expose the headless project model from M13: a collapsible left-sidebar file tree, a tab strip across the top of the editor area, and a `Ctrl+P` quick-open palette with fuzzy matching. Minimal, keyboard-driven, fast. No context menus, no drag-and-drop, no split views. This is the visible half of the project model.
**Estimated scope:** 2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 1 polished substrate; keep it simple.
- `/docs/WORKSPACE_MODEL.md` — headless data model from M13.
- `/docs/ARCHITECTURE.md` — `editor-ui` crate's role.
- `/reference/05_V2_PRD.md` — V2 visual style ("Dark Glass / Refined Civic") still applies.
- `https://docs.rs/nucleo/latest/nucleo/` OR `https://docs.rs/fuzzy-matcher/latest/fuzzy_matcher/` — fuzzy matching for quick open.

---

## The Situation In Plain English

M13 gave us a `Workspace` with a file tree and a `BufferManager` with multiple open buffers, but none of it is visible. The user can't see what files exist in the project, can't see what's open, can't jump to a file. M14 adds three small UI surfaces that turn these into first-class affordances.

The **sidebar file tree** is a collapsible panel on the left that shows the workspace's file hierarchy. Indent-per-depth, triangle icons for collapse/expand, single-click to open a file in the active tab, keyboard navigation. That's it. No right-click menus (V4+), no drag-to-move-files (V4+), no file watchers causing animated re-sorts (the tree just refreshes cleanly when files change). Toggleable via `Ctrl+B` like VS Code.

The **tab strip** sits between the sidebar and the status bar at the top of the editing area. One tab per open buffer. Active tab is highlighted. Dirty buffers show a `●` instead of the close `×`. Click to switch; click `×` to close. Keyboard: `Ctrl+Tab` / `Ctrl+Shift+Tab` cycle. Overflow (too many tabs to fit) scrolls horizontally rather than truncating names.

The **quick-open palette** is `Ctrl+P`. Modal-ish overlay in the center of the window: a single text input at the top, a scrollable list of matched files beneath. Type a few characters, fuzzy-matched file paths appear ranked by score. Up/Down to select, Enter to open, Esc to dismiss. This is the single most-used shortcut in modern editors — every developer expects it to work and be fast. We use the `nucleo` crate (the fuzzy matcher behind Helix) for the matching.

All three surfaces respect the same rule: they are keyboard-driven first, mouse-convenient second. They do not steal focus from the editor unless explicitly invoked. They render inside the same GPU frame as the editor — no separate windows, no separate event loops, no OS-native widgets.

---

## Scope

**In scope:**
- `editor-ui::Sidebar` with file tree rendering, click-to-open, keyboard nav, collapse/expand.
- `editor-ui::TabStrip` with per-buffer tabs, close buttons, dirty indicator, horizontal scroll on overflow.
- `editor-ui::QuickOpen` palette with fuzzy matching via `nucleo`.
- Keybindings: `Ctrl+B` toggle sidebar, `Ctrl+P` quick open, tab cycling (already wired in M13 keybinds, verify here).
- Sidebar width persistence (in `PersistedState` from M10).
- Active-file auto-reveal in sidebar (when switching tabs, scroll the sidebar to show the file).

**Out of scope:**
- Right-click context menus on tabs or tree nodes (V4+).
- Drag-and-drop for tabs or tree items (V4+).
- Split views / multiple editor panes (V4+).
- Command palette (like VS Code's `Ctrl+Shift+P`) — similar infrastructure to quick-open but a V4+ mission.
- Search results panel (that's M16).
- Outline / symbols view (V4+).
- Breadcrumbs (V4+).

---

## North Star

Open a project. `Ctrl+B` shows the sidebar: the file tree of your repo. Arrow keys navigate, Enter opens. Open three files. Three tabs appear at the top. `Ctrl+Tab` cycles. `Ctrl+P` opens the palette, typing `rope` shows `crates/editor-core/src/rope.rs` as the top match, Enter opens it. Everything feels instant. The whole experience is keyboard-first with clean, quiet visuals.

---

## TODO List

### 1. Add fuzzy matching dependency

- [ ] 1.1. Add `nucleo = "0.5"` (or current) to `editor-ui`. `nucleo` powers Helix's quick-open and is multithreaded + incremental.
- [ ] 1.2. Alternative considered: `fuzzy-matcher = "0.3"`. Simpler API, single-threaded. Pick `nucleo` for a 10k+-file project at 60 fps during typing; `fuzzy-matcher` is fine for < 5k files.
- [ ] 1.3. Commit: `deps(ui): add nucleo fuzzy matcher`.

### 2. Design the `Sidebar`

- [ ] 2.1. `crates/editor-ui/src/sidebar.rs`:
  ```rust
  pub struct Sidebar {
      width: f32,                          // logical pixels
      visible: bool,
      scroll_y: f32,
      expanded_dirs: HashSet<PathBuf>,     // which directories are expanded
      highlighted: Option<PathBuf>,        // keyboard-selection cursor
      flat_view: Vec<FlatEntry>,          // flattened view of visible entries
  }
  struct FlatEntry {
      path: PathBuf,
      depth: u16,
      file_type: FileType,
      display_name: String,
      is_open_buffer: bool,                // shown bolder or with a dot
  }
  ```
- [ ] 2.2. `flat_view` is rebuilt whenever the workspace tree or expanded set changes. It's a flat list of the currently-visible rows, each with its depth for indentation.
- [ ] 2.3. Rendering: each row is a `TextArea` (for the name) + optional `TextArea` (for a triangle or folder icon as a Unicode char — `▸` / `▾` for dir state).
- [ ] 2.4. Commit: `feat(ui): Sidebar data model`.

### 3. Implement Sidebar rendering

- [ ] 3.1. `Sidebar::prepare(workspace: &Workspace, buffers: &BufferManager, viewport: PhysicalSize, scale: f32) -> Vec<TextArea>`.
- [ ] 3.2. Rebuild `flat_view` if dirty. Only include children of expanded dirs. Root is always expanded.
- [ ] 3.3. For each visible row (viewport-clipped): compute `y_pixel`, build a `TextArea` with x = `depth * indent_px + icon_width + padding`, color based on `is_open_buffer` and `highlighted`.
- [ ] 3.4. Background quad: one solid quad spanning the sidebar's bounds, slightly darker than the editor background. Drawn via `QuadLayer`.
- [ ] 3.5. Body text area (main editor) gets shifted right by `sidebar.width` when visible — update `Renderer::render_frame` to account for sidebar width.
- [ ] 3.6. Commit: `feat(ui): Sidebar rendering with tree rows and background`.

### 4. Sidebar input handling

- [ ] 4.1. Add `MouseClick` / `MouseDrag` routing: check if the click is inside the sidebar bounds before routing to the editor. If yes, the sidebar consumes it.
- [ ] 4.2. Click on a triangle → toggle expand/collapse for that directory.
- [ ] 4.3. Click on a file row → send `OpenFile(path)` command, which calls `buffers.open_file(path)` and switches to it.
- [ ] 4.4. Click on a directory name → toggle expand/collapse.
- [ ] 4.5. Scroll wheel in sidebar → scroll the sidebar's `scroll_y`, not the editor.
- [ ] 4.6. Commit: `feat(ui): Sidebar mouse input`.

### 5. Sidebar keyboard navigation

- [ ] 5.1. When sidebar is focused (focus state tracked in `App`): `ArrowUp` / `ArrowDown` move the highlighted row; `ArrowRight` expands a collapsed dir; `ArrowLeft` collapses an expanded dir or moves to parent; `Enter` opens a file; `Esc` returns focus to editor.
- [ ] 5.2. `Ctrl+B` toggles the sidebar's visibility. When showing, also gives it focus so arrow keys work immediately.
- [ ] 5.3. `Ctrl+Shift+E` focuses the sidebar without toggling visibility (VS Code convention).
- [ ] 5.4. Commit: `feat(input): sidebar keyboard navigation`.

### 6. Design the `TabStrip`

- [ ] 6.1. `crates/editor-ui/src/tab_strip.rs`:
  ```rust
  pub struct TabStrip {
      height: f32,                // logical pixels
      scroll_x: f32,              // horizontal scroll for overflow
      tabs: Vec<TabVisual>,       // rebuilt each frame from BufferManager
  }
  struct TabVisual {
      id: BufferId,
      display_name: String,       // usually just the filename; full path for disambiguation if duplicate
      dirty: bool,
      is_active: bool,
      x_start: f32,
      x_end: f32,
  }
  ```
- [ ] 6.2. Tab width: compute from text width + padding. Minimum: 120 logical pixels. Maximum: 240 logical pixels (truncate with `…` past that).
- [ ] 6.3. If two open buffers share a filename (`main.rs` in two directories), disambiguate by appending the nearest unique directory: `main.rs (editor-core)` vs `main.rs (editor-render)`.
- [ ] 6.4. Commit: `feat(ui): TabStrip data model with disambiguation`.

### 7. TabStrip rendering

- [ ] 7.1. `TabStrip::prepare(buffers: &BufferManager, workspace: Option<&Workspace>, x_start: f32, viewport_width: f32) -> Vec<TextArea>`.
- [ ] 7.2. For each tab: a background quad (darker for inactive, matching editor bg for active) and two TextAreas (name + dirty/close indicator).
- [ ] 7.3. Active tab shows a top border highlight in the accent blue from the V2 design system.
- [ ] 7.4. Horizontal overflow: if the sum of tab widths exceeds viewport width, allow horizontal scroll. Mouse wheel over the tab strip scrolls horizontally.
- [ ] 7.5. Body text area (main editor) gets shifted down by `tab_strip.height`.
- [ ] 7.6. Commit: `feat(ui): TabStrip rendering`.

### 8. TabStrip input

- [ ] 8.1. Click on a tab body → switch to that buffer (`buffers.switch_to(id)`).
- [ ] 8.2. Click on the close indicator (the `×` at the right edge of each tab) → `buffers.close(id, /*force=*/false)`. If it returns `UnsavedChanges`, surface via the banner system from M08 with "Unsaved changes — press Ctrl+W again to discard" and set a flag so the next `Ctrl+W` on this buffer force-closes.
- [ ] 8.3. Middle-click on a tab → also closes (convention).
- [ ] 8.4. Scroll wheel over tab strip → horizontal scroll.
- [ ] 8.5. `Ctrl+Tab` / `Ctrl+Shift+Tab` cycle through tabs in MRU order (these are already defined in M13; wire them here).
- [ ] 8.6. Commit: `feat(ui): TabStrip mouse + keyboard input`.

### 9. Design `QuickOpen`

- [ ] 9.1. `crates/editor-ui/src/quick_open.rs`:
  ```rust
  pub struct QuickOpen {
      visible: bool,
      query: String,
      cursor: usize,                       // position within query
      matcher: nucleo::Matcher,
      results: Vec<(PathBuf, u32)>,       // (path, score)
      selected: usize,                     // index into results
      scroll: usize,                       // first visible result
  }
  impl QuickOpen {
      pub fn show(&mut self, workspace: &Workspace);
      pub fn hide(&mut self);
      pub fn update_query(&mut self, new_query: String, workspace: &Workspace);
      pub fn current_selection(&self) -> Option<&Path>;
      pub fn move_selection(&mut self, delta: i32);
      pub fn prepare(&mut self, viewport: PhysicalSize, scale: f32) -> (Vec<TextArea>, Vec<QuadDescriptor>);
  }
  ```
- [ ] 9.2. Nucleo's `Matcher::match_list` takes the query and a list of candidate strings, returns scores. Rebuild results on every query change (incremental — nucleo is fast enough).
- [ ] 9.3. For a 10k-file project, matching 3-character queries: target < 16 ms, ideally < 5 ms. Nucleo typically does 1-3 ms on that scale.
- [ ] 9.4. Result list is capped at 100 visible items; users rarely scroll past the first screen.
- [ ] 9.5. Commit: `feat(ui): QuickOpen data model with nucleo integration`.

### 10. QuickOpen rendering

- [ ] 10.1. Semi-transparent dark overlay across the entire window (alpha ~0.5) — a single quad over everything.
- [ ] 10.2. Centered card: ~500 logical pixels wide, fixed position. Background: solid dark color (slightly lighter than editor bg).
- [ ] 10.3. Top row: the query text with a blinking cursor (reuse the cursor-blink logic from M04).
- [ ] 10.4. Result list: one row per match, highlighted for the selected row. Each row shows the relative path with the matched characters highlighted in the accent blue.
- [ ] 10.5. Nucleo returns the indices of matched characters — use them to render matched characters in a brighter color within the `TextArea` (this requires multiple TextAreas per row, or per-character coloring via glyphon's per-glyph color).
- [ ] 10.6. For MVP simplicity: render the whole path in one color, without matched-char highlighting. Upgrade to per-char highlighting in M15's tree-sitter integration (which unlocks per-glyph coloring).
- [ ] 10.7. Commit: `feat(ui): QuickOpen overlay rendering`.

### 11. QuickOpen input

- [ ] 11.1. `Ctrl+P` shows the palette. While visible, keyboard input goes to the palette, not the editor:
  - Printable characters → append to query.
  - Backspace → delete last query character.
  - Arrow Up / Down → move `selected` within results.
  - Enter → `buffers.open_file(result[selected].0)`, hide palette.
  - Escape → hide palette, return focus to editor.
- [ ] 11.2. Clicking anywhere outside the palette hides it.
- [ ] 11.3. The editor's normal keybindings are suspended while palette is visible.
- [ ] 11.4. Commit: `feat(input): QuickOpen palette input`.

### 12. Active-file auto-reveal in sidebar

- [ ] 12.1. When the user switches tabs (or opens a file via quick-open), the sidebar should reveal and highlight that file.
- [ ] 12.2. Implementation: expand all ancestor directories of the file's path; set `highlighted = Some(path)`; scroll so the highlighted row is visible.
- [ ] 12.3. This should not yank keyboard focus — sidebar stays unfocused, editor keeps focus. Visual only.
- [ ] 12.4. Commit: `feat(ui): auto-reveal active file in sidebar on tab switch`.

### 13. Sidebar width persistence

- [ ] 13.1. Extend `PersistedState` from M10 with `sidebar_width: Option<f32>` and `sidebar_visible: Option<bool>`.
- [ ] 13.2. Save on exit, restore on launch.
- [ ] 13.3. Future-friendly: also add `sidebar_width` draggable splitter UI. For V3 MVP, width is fixed at 260 logical px; make the field present but use a default. Draggable splitter is one nice-to-have addition if time allows, but not required.
- [ ] 13.4. Commit: `feat(app): persist sidebar width and visibility`.

### 14. Benchmarks

- [ ] 14.1. Sidebar `prepare` cost on a 10k-entry tree with 50 visible rows: < 500 μs.
- [ ] 14.2. TabStrip `prepare` with 20 open tabs: < 100 μs.
- [ ] 14.3. QuickOpen `update_query` on a 10k-file project for a 3-character query: < 10 ms.
- [ ] 14.4. Save baseline as `m14-v3`.
- [ ] 14.5. Commit: `bench(ui): sidebar, tabs, quick-open overhead`.

### 15. Quality gates + documentation

- [ ] 15.1. Standard gates.
- [ ] 15.2. No V2 perf regressions when sidebar hidden.
- [ ] 15.3. Update `/docs/ARCHITECTURE.md` with the new UI surfaces.
- [ ] 15.4. Update `/docs/STATUS.md`: M14 complete, M15 next.
- [ ] 15.5. Tag: `git tag -a m14-complete -m "M14 complete: sidebar, tabs, quick open"`; push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. `Ctrl+B` toggles sidebar.
3. Sidebar mouse + keyboard navigation works.
4. Tabs appear for each open buffer; click to switch; close button works.
5. `Ctrl+P` opens quick-open; fuzzy match works on a 10k-file project under 16 ms.
6. Persisted sidebar state survives relaunch.
7. No V2 performance regression.
8. `m14-complete` tag pushed.

## Testing Requirements

- Unit tests on each UI data model.
- Benchmarks captured.
- Manual: open a real codebase, navigate with all three surfaces.

## Git Commit Strategy

12-14 commits. Push after items 5, 8, 11, 14.

## Handoff to M15

M15 assumes:
- Multi-buffer rendering works.
- Per-file content can be syntax-highlighted independently per tab.
- The visible UI surfaces are stable; M15 only changes how *content* is colored.

---

## Standing Orders Reminder

- Every UI element must render inside the same GPU frame as the editor. No OS-native widgets, no separate windows.
- Keyboard-first, mouse-convenient. Not mouse-first.
- Overflow is horizontal scroll, not truncation. Users must always be able to reach everything.
- Consistency with V2's "Dark Glass / Refined Civic" aesthetic: zinc-scale greys, blue accent, no emoji, no cute animations.

Go.
