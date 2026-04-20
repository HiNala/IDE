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
`docs/RISKS.md` and is carried forward past V2.

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
