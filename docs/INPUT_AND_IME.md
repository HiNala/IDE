[← docs/](./) · [README](../README.md)

# Input, keyboard, and IME

This document describes the **winit 0.30** input path in `editor-app` and the [`editor_input::map_key_event`](https://github.com/HiNala/IDE/blob/main/crates/editor-input/src/lib.rs) contract. It is the implementation reference for mission **M05** (frame loop, input pipeline, performance budgets).

## Principles

1. **Synchronous main thread:** `WindowEvent` is handled in `ApplicationHandler::window_event`. There is no queue or async step between keypress and buffer mutation.
2. **Physical keys for chords:** Shortcuts use [`KeyEvent::physical_key`](https://docs.rs/winit/latest/winit/keyboard/enum.PhysicalKey.html) so **Ctrl+Z** stays undo on non‑QWERTY layouts.
3. **Text field for typing:** Printable input uses [`KeyEvent::text`](https://docs.rs/winit/latest/winit/event/struct.KeyEvent.html) when the primary shortcut modifier (Cmd on macOS, Ctrl elsewhere) is not held—see `editor-input` implementation.
4. **Repeat:** Winit `repeat` is not filtered out; repeat behaves like repeated keypresses for movement and typing. (Undo/redo are still one command per physical press from the keymap’s `ElementState::Pressed` handling.)

## Modifier conventions

| Role | Windows / Linux | macOS |
|------|-----------------|-------|
| Primary shortcut (save, undo, copy, …) | Ctrl | Command (Super) |
| Word left/right, word delete | Ctrl | Alt |

Implemented via `primary_modifier_active()` and `word_mod()` in `editor-input`.

## `EditorCommand` overview

The full enum lives in [`crates/editor-input/src/command.rs`](../crates/editor-input/src/command.rs). M05 scoped editing commands (`InsertText`, `InsertNewline`, deletes, `ApplyCursorMotion`, undo/redo, scroll, page up/down, save, quit). Later missions added selection, clipboard, buffers, quick open, fullscreen, etc.

## IME (Input Method Editor)

### Enablement

On window creation, `editor-app` calls [`Window::set_ime_allowed(true)`](https://docs.rs/winit/latest/winit/window/struct.Window.html#method.set_ime_allowed).

### Event handling

| Event | Behavior |
|-------|----------|
| `Ime::Enabled` / `Ime::Disabled` | Redraw only; optional state reset could be added. |
| `Ime::Preedit(_, _)` | Requests a redraw. **Inline preedit is not yet merged into the rope**—the platform still composes; **commit** inserts the final text. Candidate placement uses `set_ime_cursor_area` (see below). |
| `Ime::Commit(text)` | Applies [`EditorCommand::InsertText(text)`](../crates/editor-input/src/command.rs). Sets `ime_suppress_next_keytext` so the following **keyboard** `KeyEvent` does not duplicate the same characters as typed text. |

### Dead keys / European layouts

Platforms often deliver dead-key composition through IME (`Ime::Preedit` / `Ime::Commit`) or `KeyEvent::text`. We pass `text` through when modifiers allow—see `map_key_event`.

### IME cursor area

After each successful paint, `editor-app` calls [`Window::set_ime_cursor_area`](https://docs.rs/winit/latest/winit/window/struct.Window.html#method.set_ime_cursor_area) with a rectangle derived from the caret line/column, scroll, gutter width, and `EditorRenderer::line_height_px` so the OS IME candidate window stays near the caret.

### Follow-ups

- **Visible inline preedit:** Option (a)—insert transient preedit into the buffer at the caret, or (b)—a render-only overlay—for clearer feedback during long CJK compositions.

## Keybindings (current)

**Editing**

- **Enter** — newline  
- **Tab** — four spaces (MVP)  
- **Backspace / Delete** — grapheme delete; with word modifier — word delete  
- **Arrows** — move; with Shift — extend selection  
- **Home / End** — line; with primary — buffer start/end  
- **Page Up / Down** — page motion + cursor (see app)  
- **Ctrl+arrow (Alt+arrow on macOS)** — word left/right  

**Chords (primary modifier)**

- **Z** — undo; **Shift+Z** / **Y** — redo  
- **S** — save  
- **O** — open  
- **N** / **W** / **Tab** — new / close / next buffer (when multi-buffer UI is active)  
- **C / X / V / A** — copy / cut / paste / select all  
- **P** — quick open (with workspace)  

**Other**

- **Escape** — quit (minimal; menus later)  
- **F1** — help overlay; **Ctrl+F11** — dev metrics  
- **F11** or **Alt+Enter** — fullscreen toggle  

## Debugging latency

Use **`--latency-trace`** on the `editor-app` binary: logs **input → present** time (ms) on the target `latency_trace` after the first completed frame following keyboard/IME handling. Pair with `RUST_LOG=latency_trace=info` or `info` level globally.

See also [`PERFORMANCE_BUDGETS.md`](PERFORMANCE_BUDGETS.md) and [`DIAGNOSING_PERFORMANCE.md`](DIAGNOSING_PERFORMANCE.md).

---

*Last updated: 2026-04-21 — aligns with M05 (input pipeline, IME cursor area, `map_keyboard_input`) and current `editor-app` / `editor-input`.*
