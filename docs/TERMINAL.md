# Integrated terminal (M26)

## Architecture

- **PTY:** [`portable-pty`](https://docs.rs/portable-pty/) opens a cross-platform pseudo-terminal and spawns the user’s shell.
- **Emulation:** [`alacritty_terminal`](https://docs.rs/alacritty_terminal/) maintains the screen grid; bytes from the PTY are fed through the VTE `ansi::Processor` into `Term`.
- **Rendering:** `editor-terminal::TerminalEmulator::render_snapshot()` produces row/column text runs with RGB foreground colors. `editor-render` draws them in the bottom pane via glyphon `TextArea`s (see `TextLayer::append_terminal_text_areas`).

## Frame contract

`editor_render::FrameInput` includes:

- `terminal_pane_height_px` — vertical space reserved above the status bar (0 = hidden).
- `terminal_snapshot` — optional `TerminalRenderSnapshot` from the active terminal’s emulator.

The editor document viewport height is `window_height − status_bar − terminal_pane_height_px`.

## Shell detection

`editor_terminal::detect_shell(None)` picks:

- **Unix:** `$SHELL`, else `/bin/sh`.
- **Windows:** `pwsh.exe` on PATH, else `powershell.exe`, else `%COMSPEC%`, else `cmd.exe`.

Override via `ShellConfig` when wiring settings (future).

## Keybindings (planned in app)

Per mission: `` Ctrl+` `` toggles the pane; terminal-focused keys go to `encode_key` → PTY; `Ctrl+Shift+C` / `Ctrl+Shift+V` for copy/paste when the terminal has focus.

## Agent hook

`Terminal::run_command` writes a line to the PTY and waits for a heuristic shell prompt (or times out). Intended for M20 `run_terminal`.

## Limitations (V3)

No OSC 8 hyperlinks, no sixel/graphics, no search-in-scrollback. SSH in the nested terminal is best-effort only.
