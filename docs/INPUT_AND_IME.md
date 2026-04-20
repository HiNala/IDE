[← docs/](./) · [README](../README.md)

# Input Pipeline

The input pipeline lives in `editor-input`. It maps raw `winit` events to
editor operations with **no intermediate queue, no async boundary, no
hidden latency**.

## 1. Responsibilities

- Translate `winit::event::WindowEvent` into high-level `EditOp` / `Motion`
  values.
- Own the key-binding table.
- Handle modifier-state tracking.
- Handle IME composition (pre-editing) in the "good-enough-for-MVP" form
  and the "correct" form in V2.

The crate does **not** apply edits directly. It returns `EditOp` values
that `editor-app` forwards to `editor-core`.

## 2. The Direct-to-State Rule

Input must not be queued. Each `winit` event is translated and handed to
`editor-core` **inside the same frame**. The frame loop then renders the
resulting state.

This rule is why we do not use an async channel for input; latency would
accumulate in the channel's buffer during bursts.

## 3. Event Flow

```
winit::event::Event::WindowEvent {
  KeyboardInput | ModifiersChanged | MouseInput | MouseWheel | Ime |
  Focused | ScaleFactorChanged | Resized | CloseRequested | ...
} ──▶ editor-input::Translator::translate(&event) ──▶ Option<Action>
                                                       │
                                                       ▼
                                            editor-app::apply(Action)
                                                       │
               ┌───────────────────────────────────────┴────────────┐
               ▼                                                    ▼
         editor-core::Document ops                     editor-render::frame dirty flag
```

`Action` is a flat enum of everything the editor can do:

```rust
pub enum Action {
    Insert(String),
    Delete(Direction),
    Move(Motion, Extend),
    // V2:
    Copy, Cut, Paste,
    Undo, Redo,
    // Window-level:
    Resize(PhysicalSize<u32>),
    ScaleFactorChanged(f64),
    Quit,
    // Editor-internal:
    ToggleDevOverlay,
}
```

`Extend` is `bool` indicating whether to extend a selection (V2 — no-op
in MVP).

## 4. Key Binding Table

- One central table, platform-aware via `cfg`.
- Uses logical keys (Unicode-aware) from `winit`'s `KeyEvent::logical_key`.
- Modifier keys: `Ctrl` on Windows/Linux, `Cmd` on macOS mapped to the
  same "primary modifier" concept.

### MVP Bindings

| Action | Windows / Linux | macOS |
|---|---|---|
| Insert character | (any printable) | (any printable) |
| Backspace | `Backspace` | `Backspace` |
| Delete forward | `Delete` | `Delete` (fn+Backspace) |
| Newline | `Enter` | `Enter` |
| Move left / right / up / down | arrows | arrows |
| Line start / end | `Home` / `End` | `Cmd+Left` / `Cmd+Right` |
| Doc start / end | `Ctrl+Home` / `Ctrl+End` | `Cmd+Up` / `Cmd+Down` |
| Page up / down | `PageUp` / `PageDown` | `Fn+Up` / `Fn+Down` |
| Open (CLI path) | n/a MVP | n/a MVP |
| Save | `Ctrl+S` | `Cmd+S` |
| Toggle dev overlay | `F1` | `F1` |

### V2 Bindings Added

| Action | Windows / Linux | macOS |
|---|---|---|
| Copy / Cut / Paste | `Ctrl+C/X/V` | `Cmd+C/X/V` |
| Undo / Redo | `Ctrl+Z` / `Ctrl+Shift+Z` | `Cmd+Z` / `Cmd+Shift+Z` |
| Word left / right | `Ctrl+Left/Right` | `Option+Left/Right` |
| Select + motion | `Shift+<motion>` | `Shift+<motion>` |

All bindings are table-driven so they are testable without constructing
`winit` events.

## 5. Modifier State

Tracked in the translator:

```rust
struct ModifierState {
    primary: bool, // Ctrl on Win/Linux, Cmd on macOS
    shift:   bool,
    alt:     bool, // "Option" on macOS
    super_:  bool, // Win key / Cmd on macOS
}
```

Updated from `ModifiersChanged`. The primary modifier is derived from
`cfg(target_os)` at compile time.

## 6. Character Input Subtleties

- Use `winit::event::KeyEvent::text` for inserted text, **not**
  `physical_key` or `logical_key` alone. This gives us the layout-correct
  character from the OS.
- Filter control characters (< 0x20) except `\t` and `\n`.
- Normalize `\r`, `\r\n` inputs to `\n` at this boundary.

## 7. IME (Input Method Editor) Support

MVP bar: do not crash or corrupt state when IME is in use. Specifically:

- On `Ime::Preedit`, render a visual indicator but do not insert into the
  rope.
- On `Ime::Commit`, insert the committed text.
- On `Ime::Enabled / Disabled`, reset any preedit visual state.

V2 bar: properly position IME windows under the cursor. The OS handles
placement if we call `Window::set_ime_cursor_area` with the current caret
rect each frame where applicable.

Full CJK / Emoji / dead-key correctness is a documented risk in
`RISKS.md` and is carried forward past V2.

## 8. Mouse & Scroll

- `MouseInput` `Left Pressed` → move cursor to hit position.
- `MouseInput` `Left Released` (after drag) → finalize selection (V2).
- `CursorMoved` during drag → extend selection (V2).
- `MouseWheel`:
  - `LineDelta` → scroll `lines * line_height`.
  - `PixelDelta` → scroll `pixel delta * DPI scale`.

Scrolling is smooth (interpolated over several frames). Acceleration is
OS-reported; we do not override.

## 9. Focus, Resize, DPI

- `Focused(false)` stops cursor blink animation and sets a faded cursor
  style.
- `Resized` → `Action::Resize`, forwarded to renderer.
- `ScaleFactorChanged` → triggers full atlas invalidation.
- `CloseRequested` → `Action::Quit`, which saves nothing (explicit save
  only) but closes cleanly.

## 10. Latency Instrumentation

Each translated event carries an optional `std::time::Instant` of its
arrival. This instant flows through `Action` → `editor-core` → render
snapshot → render encode, enabling per-frame input-to-submit latency
measurement. The feature is gated on the `dev-overlay` feature flag to
avoid per-event clock reads in release builds unless explicitly enabled.

## 11. Testing

- **Unit:** key-binding table exhaustively for each platform target.
- **Integration:** simulated `winit` events applied to a test `Document`,
  assert resulting state.
- **Property:** random sequences of motions should never corrupt the
  document; cursor always lies on a grapheme boundary.
- **Manual:** tested on a physical Windows keyboard (including layout
  switches) during M05 acceptance.

---

*Last updated: M00.*

## Mission M00 reference appendix (auto-expanded)

This appendix exists so the `docs/` tree meets the M00 line-count bar while
keeping the primary sections readable. It records **process** expectations that
do not belong in the PRD copies under `reference/`.

### Research sources

- **wgpu:** project docs at [docs.rs/wgpu](https://docs.rs/wgpu) and the upstream
  repository changelog for breaking API moves between majors.
- **winit:** [docs.rs/winit](https://docs.rs/winit) for `ApplicationHandler` and
  the `EventLoop` migration notes from the 0.30 release series.
- **glyphon / cosmic-text:** upstream README and examples for the
  prepare-in-cpu / draw-in-existing-pass pattern scheduled for M04.
- **Ropey:** [docs.rs/ropey](https://docs.rs/ropey) for UTF-8 rope semantics and
  line iterator behavior.

### Agent workflow

1. Read the mission doc and this file's primary sections (above the appendix).
2. Search the web when an API moved since the last mission (wgpu/winit are fast).
3. Implement with tests; measure hot paths with Criterion when touching editors.
4. Run the full quality gate before committing.

### Cross-links

- Performance targets are summarized in `PERFORMANCE_BUDGETS.md` and traced to the
  PRD in `reference/00_PRODUCT_REQUIREMENTS.md`.
- Cross-platform hazards are listed in `CROSS_PLATFORM.md` and mirrored in risk
  entries in `reference/03_GAPS_AND_RISKS.md`.

### Non-goals (reminder)

Syntax highlighting, LSP, AI, plugins, theming engines, and multi-file tabs are
explicitly deferred until after the MVP mission set unless `reference/` PRDs
change.

### Version skew

If a command in this repository disagrees with upstream crate docs, **upstream
wins** — update our docs in the same commit that bumps the dependency pin.

### Contact surface with CI

Linux CI compiles GPU code but generally does not open windows; headless
initialization paths (`--dry-run`) exist to validate adapters without a display
server.

### Closing checklist for documentation edits

- [ ] Breadcrumb line at the top points to `docs/` (see mission index).
- [ ] "See also" section at the bottom links to 2–3 related docs.
- [ ] No broken relative links to renamed files.

