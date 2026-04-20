[← docs/](./) · [README](../README.md)

# V2 PRD — Minimal Useful Editor

V2 turns the MVP from a performance-validating engine into something a
developer could actually use for a quick editing session. It adds the
smallest possible set of features that make the editor *useful*, while
inheriting every performance guarantee from the MVP.

V2 is **still not an IDE.** It is a fast, minimalist editor.

## 1. What V2 Adds

1. **Line numbers** — in a dedicated gutter, cached as its own render
   layer.
2. **Text selection** — shift-based keyboard selection and mouse drag.
3. **Clipboard integration** — OS-level copy / cut / paste.
4. **Basic undo / redo** — keybindings and short status-bar confirmation.
5. **File path visibility** — via a minimal status bar.
6. **Word-level cursor movement** — Ctrl/Option + arrow keys, Ctrl +
   Backspace/Delete.
7. **Reopen last file on launch** — using `directories` for per-user
   config.

## 2. What V2 Does Not Add

- Tabs / multi-file
- Project trees / file browser
- Syntax highlighting
- Autocomplete, LSP, AI
- Plugins, themes, settings UI
- Terminal, split panes, minimap
- Version control integration
- Collaboration or cloud sync

## 3. Interaction Model

- Keyboard is primary; mouse is secondary.
- All V2 features must feel indistinguishable in latency from MVP typing.
- Selection is rendered as GPU rectangles aligned to glyph bounds; no
  per-character draw calls.
- Undo/redo uses rope-native reversible operations with coalescing.
- Clipboard operations go through the system clipboard (`arboard` is the
  likely choice; decided in M09).

## 4. UI Composition

The frame is divided into three composited regions:

```
┌────────────┬─────────────────────────────────────────────┐
│  gutter    │           main text canvas                  │
│  (line #s) │                                             │
│            │                                             │
│            │                                             │
├────────────┴─────────────────────────────────────────────┤
│ status bar: [file path]        [row:col]   [modified]   │
└──────────────────────────────────────────────────────────┘
```

- No menus, no toolbars, no panels.
- Gutter width auto-fits the max line-number digits.
- Status bar is a single fixed-height row, rendered as its own pass.

## 5. Rendering & Performance Constraints

- Line numbers are a **separate cached glyph layer**. Their cache is
  invalidated only when the number of digits needed changes or the font
  changes.
- Selection highlights are a **single instanced quad per selection
  fragment**, batched per visible line.
- Status bar re-renders only when its state changes.
- **No V2 feature may regress any MVP performance target.** Regressions
  fail CI via the Criterion regression threshold.

## 6. Input System Additions

- Shift + navigation extends a selection anchor.
- `Ctrl+C`, `Ctrl+X`, `Ctrl+V` (Cmd on macOS) map to clipboard ops.
- `Ctrl+Z`, `Ctrl+Shift+Z` / `Ctrl+Y` map to undo/redo.
- Word-level navigation uses a simple Unicode-word-boundary iterator
  (`unicode-segmentation`'s word boundaries, narrowed).
- Input handling remains synchronous on the main thread.

## 7. File Handling Additions

- Status bar shows the active file path (relative to CWD if nested, else
  absolute).
- Save remains explicit. Atomic-write rules inherited from MVP.
- "Reopen last file" stores the path under
  `${CONFIG_DIR}/ide/last-session.toml`.
- No autosave, no backup files.

## 8. Performance Preservation

All MVP hard targets apply unchanged:

- Input-to-pixel < 5 ms.
- 60 fps minimum on scroll/edit.
- Cold start < 1 s.
- 100 MB file non-blocking open.
- Bounded memory in multi-hour sessions.

Plus V2-specific constraints:

- Gutter rendering: per-frame cost must be dominated by cache hits; cache
  miss bursts must stay under 1 ms.
- Selection rendering: O(visible selection fragments), not O(total
  selection length).

## 9. Acceptance Criteria

V2 is complete when:

- A user can open a file, select and copy text, paste, undo, redo, and
  save, with every interaction feeling immediate.
- Line numbers and cursor row/column are accurate at all times.
- Memory stays bounded through ≥ 4 hours of editing a 50 MB file.
- No crashes, freezes, or rendering artifacts in any acceptance scenario.
- Last-file persistence restores the file across restarts.

## 10. Definition of "Useful" in V2 Context

A developer could sit down with the V2 editor and edit a config file or a
small source file without frustration. It will not replace their main
editor for heavy workflows, but it will feel faster and more direct than
anything else installed on their machine.

---

*Last updated: M00.*
