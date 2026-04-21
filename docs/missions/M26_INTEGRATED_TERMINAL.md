# M26 — Integrated Terminal

**Mission ID:** M26
**Prerequisites:** M18 complete. Git baseline exists. M25 consolidated the foundation.
**Output:** A real, integrated terminal pane inside the editor. Runs the user's native shell (`cmd.exe` / `powershell.exe` on Windows, `$SHELL` on Unix), full PTY with ANSI / VT100+ support via `alacritty_terminal` and the `vte` parser, proper xterm-style color rendering. Toggleable (`` Ctrl+` ``), split horizontally from the editor area, its own font settings, resizable divider. Two terminals allowed at once (tabs in the terminal pane). Terminal is a first-class citizen in the tool API — agents run commands through the *visible* terminal via M20's `run_terminal` tool, so users see exactly what the AI is doing.
**Estimated scope:** 2-3 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 2 developer affordances.
- `/00_STATE_2026_04_20.md` — M26 lands after M18 because git + terminal together compose the core "developer workflow" surface.
- `https://docs.rs/portable-pty/` — cross-platform PTY from the WezTerm project (Apache-2.0). The only serious choice in Rust.
- `https://docs.rs/alacritty_terminal/` — VT100/xterm emulator extracted from Alacritty (Apache-2.0). Zed uses version 0.25; script-kit-gpui and others confirm it's the production-grade choice for "embed a terminal into an app."
- `https://github.com/alacritty/vte` — the VT parser (state machine from Paul Williams' spec). `alacritty_terminal` uses this internally.
- VS Code's integrated terminal is the gold-standard reference UX. Mimic the interaction model.

---

## The Situation In Plain English

A terminal is not optional in a serious IDE. Developers live in their shell; an editor that can't host one fragments the workflow. Every time they need to run a test, check `git log`, install a dependency, they have to Alt-Tab to another window — which is fine but breaks context. VS Code and Cursor solved this by integrating xterm.js into a panel beneath the editor. We do the equivalent in native Rust.

The scope of "build a terminal" sounds enormous because terminals are genuinely complex — decades of VT100/VT220/xterm standards, ANSI color codes, cursor positioning escape sequences, scrollback, selection, clipboard integration, resize, unicode+wide-character handling, modem carry-over line discipline, the works. We do not write any of this. We compose three well-tested crates:

1. **`portable-pty`** handles the cross-platform pseudo-terminal: on Windows it uses ConPTY (plus a fork `portable-pty-psmux` for modern Windows flags if needed); on Unix it uses the standard `/dev/ptmx` interface. It spawns the shell as a child process and gives us a reader/writer pair connected to that shell's stdin/stdout/stderr with proper TTY semantics (shell detects it's interactive, cargo emits colors, etc.).

2. **`alacritty_terminal` + `vte`** is the terminal emulator state machine. We feed bytes from the PTY master reader into the VTE processor; it updates an in-memory grid of `TerminalCell`s (char + foreground color + background color + flags). We read out the grid to render. This is the exact same library powering Alacritty, with Apache-2.0 licensing suitable for our project.

3. Our own renderer glues steps 1-2 into a visible pane: glyphon draws the text grid with per-cell colors (we already do per-glyph coloring for syntax highlighting in M15, so this is a natural extension). Input goes the other way: keypresses in the terminal pane encode as bytes written to the PTY master writer.

The terminal pane sits at the bottom of the editor area, above the status bar, splitting the space with the main editor. The split is a horizontal divider the user can drag. Default height: 30% of the window. `` Ctrl+` `` (VS Code convention) toggles visibility. `Ctrl+Shift+` `` (tilde with shift) creates a new terminal tab. One terminal per tab. Two tabs max for V3 (no real reason to block more, but unbounded tab creation needs a UX story we'll build later).

The *AI integration* is where this gets interesting. When M20's tool API ships, the agent gains a `run_terminal(command)` tool. Unlike a naive `Command::spawn`, this runs the command in the *visible integrated terminal*, meaning:
- The user sees exactly what the agent ran.
- Output is part of the visible session — scrollback, searchable, interruptible.
- The user can take over mid-execution if something goes wrong (`Ctrl+C`).
- The shell's environment (PATH, aliases, virtual envs) is what the user has configured, not a blank subprocess environment.

This transforms terminal integration from a convenience feature into the *primary channel* for agent system access. It's the right model for the next few years of AI-assisted development.

---

## Scope

**In scope:**
- New `editor-terminal` crate.
- `portable-pty` integration: spawn user's shell, manage PTY lifecycle, handle resize.
- `alacritty_terminal` + `vte` integration: feed PTY bytes, maintain grid state, read renderable grid.
- Terminal rendering via existing glyphon/wgpu pipeline: per-cell color, cursor rendering, selection.
- Terminal input handling: keypress → PTY byte stream with correct escape sequences.
- Terminal pane UI: bottom split, draggable divider, tab strip for up-to-2 terminals.
- Scrollback (default 10k lines), scroll with mouse wheel + `Ctrl+Home`/`Ctrl+End`.
- Copy selection (`Ctrl+Shift+C`) + paste (`Ctrl+Shift+V`) — note the Shift to avoid clashing with editor `Ctrl+C`.
- Shell detection: default to `$SHELL` on Unix, `%COMSPEC%` or `powershell.exe` on Windows, with user override in settings.
- `` Ctrl+` `` toggle; `Ctrl+Shift+` `` new terminal.
- Working directory follows the workspace root on new-terminal creation.
- Events emitted to the app: `OutputChunk`, `Exit(code)`, `TitleChanged` (for shell-set window titles).

**Out of scope:**
- Full OSC 8 hyperlink support (V4+; nice but adds complexity).
- Sixel / Kitty / iTerm2 inline graphics protocols (V4+).
- Split terminals within the pane (V4+; panes in panes is real work).
- Tmux-style session management (V4+; users already have tmux).
- Terminal search-in-scrollback (V4+; tolerable for V3).
- Multiple terminals beyond 2 tabs (V4+).
- Shell autocomplete improvements beyond whatever the shell natively does (V4+ / N/A).
- SSH connections inside the terminal — not blocked, works naturally because we just spawn the shell, but documented as unsupported for V3 (the SSH session is a second terminal inside a terminal, edge cases abound).

---

## North Star

`` Ctrl+` ``. A terminal panel slides in at the bottom. The cursor is blinking at a prompt. Type `cargo test`. Tests run, colored output scrolls. Failure hits — red line visible. `Ctrl+C` interrupts. `git status` — correct output with colors. `python -m http.server` — runs; `Ctrl+C` stops it. Everything a developer would expect from "a terminal." When M20 ships, the AI can invoke `run_terminal("cargo build")` and the same terminal runs it, visibly.

---

## TODO List

### 1. Create `editor-terminal` crate

- [ ] 1.1. `cargo new --lib crates/editor-terminal`.
- [ ] 1.2. Dependencies (pin exact versions after local compile — these ecosystems move):
  ```toml
  portable-pty = "0.8"           # or latest
  alacritty_terminal = "0.25"    # matches Zed; current stable
  vte = "0.13"                   # transitively used; also direct if we access ansi::Processor
  parking_lot = "0.12"           # Mutex for terminal state
  crossbeam-channel = "0.5"
  thiserror = "1"
  tracing = "0.1"
  ```
  Plus `editor-core`, `editor-workspace` (for shell working dir).
- [ ] 1.3. Commit: `feat(terminal): scaffold editor-terminal crate`.

### 2. PTY lifecycle

- [ ] 2.1. `src/pty.rs`:
  ```rust
  pub struct TerminalProcess {
      pty_pair: PtyPair,
      child: Box<dyn Child + Send + Sync>,
      writer: Box<dyn Write + Send>,
      reader_thread: Option<JoinHandle<()>>,
      output_tx: Sender<Vec<u8>>,
  }
  impl TerminalProcess {
      pub fn spawn(shell: ShellConfig, cwd: PathBuf, size: PtySize) -> Result<(Self, Receiver<Vec<u8>>), TerminalError>;
      pub fn resize(&mut self, size: PtySize) -> Result<(), TerminalError>;
      pub fn write(&mut self, bytes: &[u8]) -> Result<(), TerminalError>;
      pub fn kill(&mut self) -> Result<(), TerminalError>;
      pub fn wait(&mut self) -> Option<ExitStatus>;
  }
  pub struct ShellConfig {
      pub program: PathBuf,         // /bin/bash, C:\Windows\System32\cmd.exe, etc.
      pub args: Vec<String>,
      pub env: HashMap<String, String>,
  }
  ```
- [ ] 2.2. Shell detection:
  - Unix: read `$SHELL`; fallback to `/bin/sh`.
  - Windows: prefer `pwsh.exe` if on PATH; else `powershell.exe`; else `%COMSPEC%` which is typically `cmd.exe`.
  - Respect user override from settings (plumbed by M28).
- [ ] 2.3. Reader thread: spawn a thread that reads from `pair.master` in 8 KB chunks and forwards to `output_tx`. On read error or EOF, send final chunk and exit.
- [ ] 2.4. Commit: `feat(terminal): PTY lifecycle with portable-pty`.

### 3. VTE + alacritty_terminal integration

- [ ] 3.1. `src/emulator.rs`:
  ```rust
  pub struct TerminalEmulator {
      term: Term<NopEventListener>,
      processor: vte::ansi::Processor,
      size: TerminalSize,
      title: String,
  }
  impl TerminalEmulator {
      pub fn new(size: TerminalSize, scrollback: usize) -> Self;
      pub fn process_bytes(&mut self, bytes: &[u8]);
      pub fn resize(&mut self, new_size: TerminalSize);
      pub fn grid(&self) -> &Grid<Cell>;
      pub fn cursor(&self) -> Point;
      pub fn title(&self) -> &str;
      pub fn selection_to_string(&self) -> Option<String>;
      pub fn set_selection(&mut self, start: Point, end: Point);
      pub fn clear_selection(&mut self);
      pub fn scroll(&mut self, lines: i32);
  }
  ```
- [ ] 3.2. `NopEventListener` is a placeholder `EventListener` impl that does nothing (alacritty_terminal emits events for bell, title change, etc.; we initially ignore them except title).
- [ ] 3.3. Default `TermConfig` with `scrolling_history: 10_000`.
- [ ] 3.4. Threading: `Term` is not Send by default; wrap in `parking_lot::Mutex<TerminalEmulator>`. The reader thread pushes bytes to the emulator through the mutex.
- [ ] 3.5. Commit: `feat(terminal): VTE parser + alacritty_terminal state`.

### 4. Glue: `Terminal` = PTY + Emulator

- [ ] 4.1. `src/terminal.rs`:
  ```rust
  pub struct Terminal {
      id: TerminalId,
      process: TerminalProcess,
      emulator: Arc<Mutex<TerminalEmulator>>,
      dirty: AtomicBool,
      title: String,
  }
  impl Terminal {
      pub fn new(config: TerminalConfig) -> Result<Self, TerminalError>;
      pub fn input(&mut self, bytes: &[u8]) -> Result<(), TerminalError>;
      pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError>;
      pub fn poll_output(&self) -> bool;        // returns true if dirty; clears the flag
      pub fn lock_emulator(&self) -> impl DerefMut<Target = TerminalEmulator>;
      pub fn kill(&mut self);
  }
  ```
- [ ] 4.2. A "pump" thread per terminal: reads from `TerminalProcess`'s output channel, locks emulator, calls `process_bytes`, sets dirty flag. Keep this thread cheap.
- [ ] 4.3. Commit: `feat(terminal): Terminal type combining PTY and emulator`.

### 5. Rendering integration

- [ ] 5.1. New `editor-render::TerminalLayer`:
  - Input: an `Arc<Mutex<TerminalEmulator>>`, viewport origin, cell size (from font metrics), scroll offset.
  - Renders: background quads for each cell whose `bg` isn't the default; text via glyphon `TextArea`s grouped by run of same foreground color (so a typical line is ~5-15 TextAreas, not hundreds).
  - Cursor: solid quad overlay at `grid.cursor()` position.
  - Selection: translucent overlay quad.
- [ ] 5.2. Font: same JetBrains Mono as the editor. Terminal font size configurable independently (default same as editor).
- [ ] 5.3. The layout cache from M04/M25's fix applies here too — cache terminal-line shaping keyed by `(row_idx, content_hash, scroll_offset)`.
- [ ] 5.4. Performance target: terminal panel rendering adds < 2 ms to frame time at 80×24 with busy output.
- [ ] 5.5. Commit: `feat(render): TerminalLayer`.

### 6. Input encoding

- [ ] 6.1. `src/input.rs`:
  ```rust
  pub fn encode_key(event: &KeyEvent, modifiers: ModifiersState) -> Option<Vec<u8>>;
  ```
- [ ] 6.2. Translate winit keyboard events to the bytes a shell expects:
  - Printable chars → UTF-8 bytes.
  - Enter → `\r` (shells expect carriage return).
  - Backspace → `\x7f` (DEL) by convention; `\b` (BS) as fallback if shell configured for it.
  - Tab → `\t`.
  - Arrows → `\x1b[A` / `[B` / `[C` / `[D`. With modifiers, the xterm-extended encoding `\x1b[1;5A` for `Ctrl+Up` etc.
  - Home/End/PageUp/PageDown → standard escape sequences.
  - `Ctrl+<letter>` → the corresponding control byte (`Ctrl+C` → `\x03`).
  - Esc → `\x1b`.
- [ ] 6.3. Windows note: ConPTY handles key translation somewhat differently; `portable-pty` with the `WIN32_INPUT_MODE` flag is best. Verify Ctrl+C interrupts correctly on Windows (this is historically finicky — test against `ping -t` to confirm).
- [ ] 6.4. Unit tests: one test per key class.
- [ ] 6.5. Commit: `feat(terminal): winit key → PTY byte encoding`.

### 7. Terminal pane UI

- [ ] 7.1. `editor-ui::TerminalPane`:
  - Bottom-of-window panel with configurable height (default 30% of window).
  - Draggable top edge: divider between editor area and terminal. Min height: 4 lines; max: 90% of window.
  - Tab strip across the top of the pane for up-to-2 terminals; close button per tab.
  - The actively focused terminal is which one gets keystrokes.
  - Toolbar on right: new-terminal (`+`), close-pane (`×`), settings gear (→ settings from M28).
- [ ] 7.2. Focus model: click in terminal → terminal takes focus. Click in editor → editor takes focus. Terminal shows a subtle focus ring when active.
- [ ] 7.3. Commit: `feat(ui): TerminalPane with draggable split and tab strip`.

### 8. Keybindings

- [ ] 8.1. `` Ctrl+` `` — toggle pane visibility.
- [ ] 8.2. `` Ctrl+Shift+` `` — new terminal in pane.
- [ ] 8.3. `Ctrl+Shift+C` — copy selection (while terminal focused; `Ctrl+C` is reserved for sending SIGINT).
- [ ] 8.4. `Ctrl+Shift+V` — paste from clipboard.
- [ ] 8.5. `Ctrl+PgUp` / `Ctrl+PgDn` — switch between terminal tabs.
- [ ] 8.6. Everything else while terminal focused goes through `encode_key` to the PTY.
- [ ] 8.7. Commit: `feat(input): terminal keybindings`.

### 9. Resize handling

- [ ] 9.1. When the pane is resized (via divider drag or window resize), compute new `cols`/`rows` from pane pixel dimensions and cell size. Call `terminal.resize(cols, rows)`.
- [ ] 9.2. portable-pty: `pair.master.resize(PtySize { ... })`. This fires `SIGWINCH` on Unix so shells and TUI apps reflow.
- [ ] 9.3. alacritty_terminal: `term.resize(&new_size)`.
- [ ] 9.4. Debounce: during a continuous window drag, don't thrash PTY resizes; coalesce to one per 50 ms.
- [ ] 9.5. Test: run `htop` or `top`, resize the pane, verify TUI reflows correctly.
- [ ] 9.6. Commit: `feat(terminal): coordinated resize across PTY and emulator`.

### 10. Selection + clipboard

- [ ] 10.1. Mouse drag inside the terminal → select a rectangular range of cells. Use `term.set_selection`.
- [ ] 10.2. Selection renders as a translucent overlay.
- [ ] 10.3. `Ctrl+Shift+C` → `emulator.selection_to_string()` → `arboard` clipboard.
- [ ] 10.4. Commit: `feat(terminal): mouse selection + copy to clipboard`.

### 11. Event plumbing to the app

- [ ] 11.1. Terminal emits events on a channel the app polls:
  - `Dirty` — needs redraw.
  - `TitleChanged(String)` — shell set a new window title (via OSC 0/2).
  - `Exited(ExitStatus)` — shell died.
  - `Bell` — terminal bell ring (show a visual flash on the terminal tab, don't play audio for V3).
- [ ] 11.2. On `Exited`, the tab shows `(exited)` until the user closes it, so they can read final output.
- [ ] 11.3. Commit: `feat(terminal): event plumbing to app`.

### 12. Working directory

- [ ] 12.1. Default CWD for a new terminal: the workspace root from M13.
- [ ] 12.2. `Ctrl+Shift+~` (new terminal shortcut) opens one in workspace root. No per-file CWD for V3.
- [ ] 12.3. Post-V3: OSC 7 escape (shell-reported CWD) can track working directory changes, but V3 doesn't need it.
- [ ] 12.4. Commit: `feat(terminal): default CWD from workspace root`.

### 13. AI hooks (preparation for M20)

- [ ] 13.1. `Terminal::run_command(command: &str) -> oneshot::Receiver<CommandResult>`:
  - Writes `command + "\n"` to the PTY.
  - Captures output from that point until the shell's next prompt is detected (prompt detection is heuristic: watch for the shell's `$ ` / `> ` / `PS C:\...>` pattern at start of a line). On prompt detection, resolve with captured output.
  - Returns an error if no prompt appears within configurable timeout (default 30s).
- [ ] 13.2. Alternative (cleaner) approach: configure the shell to emit OSC 133 command prompts ("pre-prompt" marker) — Zed, Warp, and iTerm use this. Can be added as a V4+ enhancement; heuristic detection is fine for V3.
- [ ] 13.3. This `run_command` is what M20's `run_terminal` tool will call.
- [ ] 13.4. Commit: `feat(terminal): run_command API for agent integration`.

### 14. Performance + stress

- [ ] 14.1. `yes` / `cat /dev/urandom | head -c 10M` test: terminal doesn't hitch; output scrolls. Target: sustain 10 MB/s of output into the emulator without dropping frames in the editor.
- [ ] 14.2. `htop` / `top` test: TUI renders correctly, cursor in right place, colors correct.
- [ ] 14.3. `vim` or `nvim` test: works acceptably. (Running an editor inside the editor's terminal is a corner case, but it's a great stress test for the emulator.)
- [ ] 14.4. CI: add a smoke test that spawns a terminal, runs `echo hello`, captures output, asserts correctness.
- [ ] 14.5. Commit: `test(terminal): stress and interop tests`.

### 15. Quality gates + documentation

- [ ] 15.1. Standard gates.
- [ ] 15.2. `/docs/TERMINAL.md` describing architecture, shell detection, keybindings, known limitations, and the `run_command` API.
- [ ] 15.3. Tag: `git tag -a m26-complete -m "M26 complete: integrated terminal"`. Push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. `` Ctrl+` `` toggles the pane; pane renders a working shell.
3. All common keybindings work; `Ctrl+C` interrupts correctly.
4. Selection + copy + paste work.
5. `htop` / `vim` / `cargo test` all render correctly.
6. Resize (divider drag + window resize) works without visual glitches or TUI breakage.
7. No V2.1 perf regression when terminal pane is hidden.
8. `m26-complete` tag pushed.

## Testing Requirements

- Unit tests on `encode_key`.
- Smoke test: spawn terminal, echo, assert.
- Manual: run the stress cases from item 14.

## Git Commit Strategy

12-14 commits. Push after items 4, 6, 10, 13, 15.

## Handoff to M19

M19 assumes the terminal exists and is stable. M20's `run_terminal` tool will call `Terminal::run_command`. The tool sees the terminal as the "system side" of agent actions.

---

## Standing Orders Reminder

- Do not write a terminal emulator. We use `alacritty_terminal`.
- Never block the render thread on PTY I/O. All PTY reading happens on the dedicated pump thread.
- Windows ConPTY is finicky. When tests fail on Windows specifically, suspect the PTY layer first, not your code.
- The terminal is the user's native shell running the user's native configuration. Do not try to be clever and pre-process commands — the whole point is that it's *their* terminal.

Go.
