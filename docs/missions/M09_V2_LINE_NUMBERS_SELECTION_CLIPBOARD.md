# M09 — V2: Line Numbers, Selection, Clipboard, Undo/Redo UI

**Mission ID:** M09
**Prerequisites:** M08 complete. `0.1.0-mvp` tag is pushed. Every MVP acceptance item is green.
**Output:** The editor transitions from "architecture demo" to "actually usable tool." A line number gutter appears on the left. Selection highlights render visibly. Mouse click positions the cursor and mouse drag extends the selection. The system clipboard works (Ctrl+C / Ctrl+X / Ctrl+V, `Cmd` on macOS). Shift-arrow keys extend selection. Undo/redo are fully wired into the keybindings and work for selection-aware operations.
**Estimated scope:** 2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/reference/05_V2_PRD.md` §4 and §5 — the V2 feature list and UI philosophy.
- `/docs/ARCHITECTURE.md` — `editor-ui` crate's responsibility (where gutter + selection rendering lives).
- `/docs/INPUT_AND_IME.md` — keybindings and mouse event handling.
- `https://docs.rs/arboard/latest/arboard/` — cross-platform clipboard API.

---

## The Situation In Plain English

The MVP worked. It was fast, correct, and cleanly architected — but a developer could not realistically use it for a morning of actual work because essentials were missing. No line numbers meant no easy way to navigate to "the bug on line 247." No mouse selection meant you couldn't grab a paragraph with the cursor. No clipboard meant you couldn't copy code between files. These aren't features we're proud to skip — they're the absolute minimum to cross from "demo" to "tool." M09 delivers them, staying strict about not letting them balloon into a full IDE feature set.

The key discipline this mission is doing the minimum usable version of each feature, not the ideal one. Line numbers are a simple monospaced column of digits on the left of the text. Selection highlighting is a translucent quad under every line of selected text. Mouse selection is click = place cursor, drag = extend selection, double-click = select word, triple-click = select line. Clipboard integration is the three standard shortcuts. No custom cursor styles yet, no drag-and-drop, no rectangular block selection, no multi-cursor. Those are legitimate V2+ features but they are explicitly not this mission.

The V2 PRD warns against "architectural drift" — the temptation to add one more abstraction to make the UI layer "flexible." We resist. Line numbers go into `editor-ui` as a thin `GutterLayer`. Selection highlighting goes into `editor-render` as a `SelectionLayer` reusing the existing `QuadLayer` primitive. Mouse input extends the `KeyMap` into an `InputMap` that handles both. Clipboard goes into `editor-app` as a direct call into `arboard`. Four small, targeted additions, each justified by a specific V2 line item.

---

## Scope

**In scope:**
- Line number gutter rendered by a new `editor-ui::GutterLayer` that uses `editor-render::TextLayer` for its digit text.
- Selection highlighting rendered as translucent quads via a new `editor-render::SelectionLayer` built on `QuadLayer`.
- Mouse input: `MouseInput`, `CursorMoved`, `MouseWheel` wired into the input pipeline.
- Click-to-position, drag-to-select, double-click-to-select-word, triple-click-to-select-line.
- Shift+arrow keys extend selection (the `EditorCommand::ExtendSelection` variant already exists from M05; this mission wires it to keybindings and renders the result).
- Clipboard: `Ctrl+C` (or `Cmd+C`) = copy selection; `Ctrl+X` = cut selection; `Ctrl+V` = paste.
- `Ctrl+A` = select all.
- Undo/redo coalescing-aware: selecting, then typing, should undo into a single logical step if possible.

**Out of scope:**
- Finding/replacing (post-V2).
- Multiple tabs (post-V2).
- Line wrapping (V2+ backlog).
- Multi-cursor / column selection (post-V2).
- Syntax highlighting (post-V2).
- Drag-and-drop of text or files (post-V2).
- Autosave / auto-backup (post-V2).

---

## North Star

At the end of M09, you can:

- Open a 5000-line file.
- See `1`, `2`, `3`, ... down the left margin, right-aligned, in the same font but slightly dimmer color.
- Click at line 247 column 10 — cursor moves there.
- Hold Shift and click line 260 column 0 — selection spans ~13 lines, highlighted.
- Press Ctrl+C — the selected text is on the system clipboard; pasting into Notepad / VS Code / TextEdit delivers the expected content.
- Ctrl+V into the editor — the text is inserted at the cursor.
- Ctrl+Z undoes the paste in one step.
- Feel no frame-rate regression vs. the MVP baseline.

---

## TODO List

### 1. Extend `Selection` semantics in `editor-core`

- [ ] 1.1. Review `Selection` from M02. It already has `anchor` and `head`. Add convenience methods: `is_forward(self) -> bool`, `swap(&mut self)`, `contains(self, pos: BytePos) -> bool`.
- [ ] 1.2. Add helper: `line_range_inclusive(&self, buffer: &TextBuffer) -> (usize, usize)` returning `(first_line, last_line)` covered by the selection — used by the selection renderer to iterate lines.
- [ ] 1.3. Commit: `feat(core): extend Selection with helpers for rendering`.

### 2. Build the `GutterLayer`

- [ ] 2.1. New crate `editor-ui` (scaffolded in M01, empty so far). Add `editor-render` and `editor-core` as deps.
- [ ] 2.2. `crates/editor-ui/src/gutter.rs`:
  ```rust
  pub struct GutterLayer {
      /// Width in logical pixels. Derived from max line number's digit count.
      width: f32,
      /// Cached TextAreas for visible line numbers. Rebuilt on scroll/resize.
      text_areas: Vec<TextArea>,
  }
  impl GutterLayer {
      pub fn new(font_size: f32) -> Self;
      pub fn prepare(&mut self, total_lines: usize, first_visible: usize, last_visible: usize, line_height: f32, scroll_y: f32, text_layer: &mut TextLayer) -> Result<(), RenderError>;
      pub fn width(&self) -> f32;
      pub fn render<'a>(&'a self, pass: &mut RenderPass<'a>) -> Result<(), RenderError>;
  }
  ```
- [ ] 2.3. `prepare`: compute gutter width from `total_lines.ilog10() + 1` digits * monospaced digit width + margin. For each visible line `n`, create a right-aligned TextArea with `format!("{}", n + 1)` in a slightly dimmer color than the body text (e.g., `Color::rgb(0x80, 0x80, 0x80)` vs body's `0xE0`).
- [ ] 2.4. `render`: just delegates to `text_layer.render(pass)` — glyphon already has everything it needs from `prepare`.
- [ ] 2.5. Body text must shift right by `gutter.width()` — update `TextLayer::prepare` to accept an `x_offset: f32` and add it to each body `TextArea`'s `left`. The gutter's TextAreas use `left: 0`.
- [ ] 2.6. Unit test: `GutterLayer::width` for various line counts (1, 10, 100, 1000, 10000).
- [ ] 2.7. Commit: `feat(ui): add GutterLayer for line numbers`.

### 3. Build the `SelectionLayer`

- [ ] 3.1. `crates/editor-render/src/selection.rs`. Uses the existing `QuadLayer` primitive from M04.
- [ ] 3.2. `SelectionLayer::prepare(&mut self, selection: Selection, buffer: &TextBufferSnapshot, metrics: Metrics, scroll: ScrollOffset, gutter_width: f32, viewport: PhysicalSize<u32>)` computes one quad per line of selection:
  - If selection is single-line: one quad from `start_x` to `end_x` on that line.
  - If selection spans multiple lines: first line from `start_x` to end-of-line, middle lines full width, last line from start-of-line to `end_x`.
  - Color: `Color::rgba(0x30, 0x50, 0x90, 0x80)` — translucent blue. Alpha is key; the quad sits under the text.
- [ ] 3.3. `render`: instanced draw of all quads via `QuadLayer`.
- [ ] 3.4. Blend state: `QuadLayer`'s pipeline must use `BlendState::ALPHA_BLENDING` for transparency. Update if not already.
- [ ] 3.5. Render order: `SelectionLayer::render` runs *before* `TextLayer::render` so the highlight sits beneath the glyphs. `CursorLayer`/`QuadLayer`-for-cursor runs *after* text so the cursor sits on top.
- [ ] 3.6. Unit test: selection spanning 3 lines produces 3 quads with correct bounds.
- [ ] 3.7. Commit: `feat(render): add SelectionLayer with per-line translucent quads`.

### 4. Wire gutter and selection into `Renderer`

- [ ] 4.1. `Renderer` gains `gutter: GutterLayer` and `selection_layer: SelectionLayer`.
- [ ] 4.2. `FrameInput` gains a `selection: Selection` field (in addition to the existing `cursor`).
- [ ] 4.3. `render_frame` order: `selection_layer.prepare` → `gutter.prepare` → `text_layer.prepare` → acquire texture → begin pass → `selection_layer.render` → `text_layer.render` → `gutter.render` (actually the gutter text is drawn by the same TextLayer — double-check ordering) → cursor quad → end pass → present.
- [ ] 4.4. Hmm — the gutter's line-number digits are TextAreas submitted to the same TextLayer, meaning a single `TextLayer::prepare` call should collect body + gutter TextAreas. Restructure: `TextLayer::prepare` accepts a `Vec<TextArea>` assembled by the caller (Renderer), where body TextAreas come first and gutter TextAreas come second. The caller (Renderer) is responsible for assembling them; `GutterLayer` becomes a helper that *produces* TextAreas rather than *owning* a TextLayer. This keeps TextLayer unified and avoids two separate text-render passes.
- [ ] 4.5. Update M04's `TextLayer::prepare` signature accordingly. Minor refactor; re-run benchmarks to make sure no perf regression.
- [ ] 4.6. Commit: `refactor(render): assemble body + gutter TextAreas in Renderer; render selection below text`.

### 5. Add mouse input handling

- [ ] 5.1. Extend `editor-input::EventTranslator::translate` to handle `WindowEvent::MouseInput`, `WindowEvent::CursorMoved`, `WindowEvent::MouseWheel`.
- [ ] 5.2. Mouse state machine in `EventTranslator`:
  ```rust
  pub struct MouseState {
      pressed: bool,
      last_click: Option<(Instant, PhysicalPosition<f64>)>,
      click_count: u8,  // 1 = single, 2 = double, 3 = triple
  }
  ```
- [ ] 5.3. On `MouseInput { state: Pressed, button: Left, .. }`:
  - If another click within 500 ms and within 4 physical pixels of the last click: increment `click_count`. Else reset to 1.
  - Emit `EditorCommand::MouseClick { position, click_count, modifiers }`.
  - Mark `pressed = true`.
- [ ] 5.4. On `CursorMoved { position, .. }` while `pressed`:
  - Emit `EditorCommand::MouseDrag { position, modifiers }`.
- [ ] 5.5. On `MouseInput { state: Released, .. }`:
  - `pressed = false`. No command emitted.
- [ ] 5.6. On `MouseWheel { delta, phase, .. }`:
  - Emit `EditorCommand::Scroll(ScrollDelta::Pixels(dy))` with proper pixel conversion for `LineDelta` vs `PixelDelta`.
- [ ] 5.7. Add `MouseClick` and `MouseDrag` variants to `EditorCommand`.
- [ ] 5.8. Unit tests: single → double → triple click detection; drag state transitions; wheel event mapping.
- [ ] 5.9. Commit: `feat(input): handle mouse click, drag, wheel events`.

### 6. Implement click-to-position and drag-to-select in `EditorState`

- [ ] 6.1. New method `EditorState::mouse_click(&mut self, pos: PhysicalPosition<f64>, click_count: u8, shift_held: bool, layout_hint: LayoutHint)`.
- [ ] 6.2. `LayoutHint` is a small struct passed in from the rendering layer containing: `scroll_y`, `line_height`, `gutter_width`, `viewport_size`. This is the bridge that lets `EditorState` convert from pixels back to a `BytePos` without knowing about `TextLayer`'s internals.
- [ ] 6.3. Pixel → `BytePos` conversion:
  - y → line: `line = ((pos.y + scroll_y) / line_height).floor()`.
  - x → column: for monospaced MVP, `col = ((pos.x - gutter_width) / char_width).floor()`. (Real proportional-font column math is V2+ because we're monospaced-only.)
  - `(line, col) → BytePos` via `TextBuffer::line_col_to_byte`.
- [ ] 6.4. `click_count == 1`:
  - Without Shift: cursor moves to clicked position; selection collapses.
  - With Shift: selection extends to clicked position (anchor stays).
- [ ] 6.5. `click_count == 2`: select the word at the clicked position. Use `WordLeft` / `WordRight` cursor motion logic from M05.
- [ ] 6.6. `click_count == 3`: select the whole line.
- [ ] 6.7. `EditorState::mouse_drag(&mut self, pos: PhysicalPosition<f64>, layout_hint: LayoutHint)`:
  - Convert pos to `BytePos`.
  - Update `selection.head`; keep `selection.anchor`.
- [ ] 6.8. Auto-scroll on drag near viewport edge: if `pos.y < 20px` or `pos.y > viewport_height - 20px`, adjust `scroll.y_px` by `line_height` per frame.
- [ ] 6.9. Unit tests: click at various positions produces correct cursor placement; drag produces correct selection.
- [ ] 6.10. Commit: `feat(app): click/drag cursor placement and selection`.

### 7. Wire Shift+arrow for selection extension

- [ ] 7.1. In `editor-input::KeyMap`, when `Shift` is held and a movement key is pressed, emit `EditorCommand::ExtendSelection(motion)` instead of `MoveCursor(motion)`.
- [ ] 7.2. In `EditorState::apply`, handle `ExtendSelection(motion)`: apply motion to cursor, adjust `selection.head`, keep `selection.anchor`.
- [ ] 7.3. Movement *without* shift while a selection is active should collapse selection to the cursor before moving (matching VS Code behavior).
- [ ] 7.4. Commit: `feat(input, app): Shift+arrow extends selection; plain arrow collapses`.

### 8. Wire clipboard

- [ ] 8.1. `editor-app/src/clipboard.rs`:
  ```rust
  use arboard::Clipboard;
  pub struct ClipboardManager { inner: Option<Clipboard> }
  impl ClipboardManager {
      pub fn new() -> Self { /* Clipboard::new() may fail; tolerate it */ }
      pub fn copy(&mut self, s: &str) -> Result<(), ClipboardError>;
      pub fn paste(&mut self) -> Result<String, ClipboardError>;
  }
  ```
- [ ] 8.2. Add `arboard` to `editor-app` deps.
- [ ] 8.3. `EditorState` gains a `clipboard: ClipboardManager` field.
- [ ] 8.4. Handle commands:
  - `Copy` (Ctrl+C): if selection non-empty, `buffer.slice_to_string(selection.range())?` → `clipboard.copy(...)`. Leave buffer unchanged.
  - `Cut` (Ctrl+X): same as copy, then delete selection (as a regular edit that goes through undo).
  - `Paste` (Ctrl+V): `clipboard.paste()?` → `apply_edit(Insert { pos: cursor, text })`. If there's a selection, first delete it (as a single undoable compound edit — see UndoStack checkpoint logic in M02).
- [ ] 8.5. Add `Copy`, `Cut`, `Paste`, `SelectAll` variants to `EditorCommand`. Wire them in `KeyMap`.
- [ ] 8.6. `Ctrl+A` → `SelectAll`: set `selection = Selection { anchor: BytePos(0), head: BytePos(buffer.len_bytes()) }`.
- [ ] 8.7. Platform-aware modifier: `Cmd+C` on macOS. This was already set up in M05's `PlatformMod`; just make sure it's applied here.
- [ ] 8.8. Unit tests with a fake clipboard (`ClipboardManager` accepts a `Box<dyn ClipboardBackend>` so tests can inject a `Vec<String>`-backed fake). This keeps unit tests hermetic; integration tests on a real OS clipboard are manual.
- [ ] 8.9. Manual verification: copy from editor, paste into Notepad (Windows) / TextEdit (macOS) / xclip (Linux). Copy from those and paste into editor. Confirm Unicode round-trips.
- [ ] 8.10. Commit: `feat(app): clipboard integration with arboard`.

### 9. Cut/paste and undo

- [ ] 9.1. Cut + paste should be a single undoable step each. This means the delete (from cut) is one undo group, and the insert (from paste) is another. Pasting over a selection (selection delete + insert) should be one undo group containing both edits.
- [ ] 9.2. Use `UndoStack::checkpoint()` around compound operations. The `Cut` command: checkpoint → delete → checkpoint. The `Paste`-over-selection command: checkpoint → delete selection → insert clipboard → checkpoint.
- [ ] 9.3. Verify with proptest: random sequences of cut/paste/undo round-trip correctly.
- [ ] 9.4. Commit: `feat(core, app): group cut/paste operations into single undo steps`.

### 10. Polish: cursor visible during selection

- [ ] 10.1. The blinking cursor is annoying during active selection. Hide the cursor when `selection.is_empty()` is false. Restore when selection collapses.
- [ ] 10.2. Commit: `polish(render): hide cursor blink during active selection`.

### 11. Benchmarks

- [ ] 11.1. Benchmark `SelectionLayer::prepare` for short and long selections.
- [ ] 11.2. Benchmark `GutterLayer` contribution to TextLayer's TextArea count (impact on `TextLayer::prepare`).
- [ ] 11.3. Benchmark mouse drag → cursor update → render cycle: target < 5 ms end-to-end.
- [ ] 11.4. Save baseline as `m09-v2`.
- [ ] 11.5. Commit: `bench(ui, render): selection and gutter overhead`.

### 12. Cross-platform verification

- [ ] 12.1. Mouse drag on Windows, macOS, Linux — same feel, same timing for double-click detection.
- [ ] 12.2. Clipboard with Unicode (CJK, emoji, RTL) round-trips on each OS.
- [ ] 12.3. Gutter renders cleanly at 1×, 1.25×, 1.5×, 2× DPI.
- [ ] 12.4. Commit: `fix: cross-platform adjustments from V2 verification`.

### 13. Quality gates + documentation

- [ ] 13.1. All standard quality gates.
- [ ] 13.2. Update `/docs/ARCHITECTURE.md` with the gutter + selection + mouse additions.
- [ ] 13.3. Update `/docs/INPUT_AND_IME.md` with the mouse state machine.
- [ ] 13.4. Update `/docs/STATUS.md`: M09 complete, M10 next.
- [ ] 13.5. Update `/CHANGELOG.md` with V2 additions.
- [ ] 13.6. Tag: `git tag -a m09-complete -m "M09 complete: line numbers, selection, clipboard"`; push.

---

## Validation / Acceptance Criteria

M09 is complete when:

1. Quality gates pass.
2. Line numbers render correctly for buffers from 1 to 10000+ lines.
3. Click positions cursor; drag selects; double-click selects word; triple-click selects line.
4. Shift+arrow extends selection; plain arrow collapses.
5. Copy / cut / paste work with the system clipboard; round-trip Unicode intact.
6. Ctrl+A selects all.
7. Undo collapses cut or paste into a single step.
8. No MVP-level performance regression vs. `m08-mvp` baseline (verified via Criterion gate from M07).
9. `m09-complete` tag pushed.

## Testing Requirements

- Unit tests for mouse state machine, click-to-position, selection rendering, clipboard (via fake backend).
- Property tests: random cut/copy/paste/undo sequences preserve invariants.
- Manual cross-platform verification.

## Git Commit Strategy

12-16 commits. Push after items 2, 3, 4, 6, 8, 10, 13.

## Handoff to M10

M10 assumes:

- Line numbers, selection, clipboard, shift-arrow, mouse, undo/redo are stable.
- The editor is usable for a working session.
- M10 adds the remaining V2 polish: word navigation, status bar, persistence, and the final V2 acceptance pass.

---

## Standing Orders Reminder

- Keep each feature minimal. Line numbers are *just* numbers. Selection is *just* a translucent quad. Don't add minimap, folding, ruler lines, or indent guides — those are post-V2.
- Every V2 feature must not regress MVP performance. If a selection highlight regresses frame time, fix the regression or simplify the feature.
- Test clipboard cross-platform. It is the single most fragile piece of OS integration.

Go.
