# API design notes (`editor-core`)

**Status:** The byte-centric public API is **implemented** in `crates/editor-core`. This file records where the canonical description lives.

## Canonical references

- **[TEXT_ENGINE.md](TEXT_ENGINE.md)** — responsibilities, type diagram, byte vs grapheme, Undo model.
- **Rustdoc** on `editor_core` (`cargo doc -p editor-core --no-deps`) — per-type/method contracts.
- **Mission brief:** [`missions/M02_TEXT_ENGINE.md`](missions/M02_TEXT_ENGINE.md) — historical TODO list; implementation may include follow-ups (e.g. `word_nav`, `WorkerPool`).

## Design choices (fixed)

| Topic | Decision |
|-------|----------|
| Canonical coordinate | **UTF-8 byte offset** (`BytePos` in `crates/editor-core/src/position.rs`) |
| Line/column | Derived via `TextBuffer::byte_to_line_col` / `line_col_to_byte`; column = UTF-8 bytes within line |
| Cursor lateral motion | **Grapheme clusters** (`unicode-segmentation`), not raw codepoints |
| Rope | [`ropey::Rope`](https://docs.rs/ropey) behind `TextBuffer` (`buffer/mod.rs`); no public `Rope` in API |
| Undo | `Edit` / `EditKind` + `UndoStack` with insert coalescing |

New public surface should extend these rules unless a mission explicitly revisits them.
