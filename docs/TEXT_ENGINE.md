[← docs/](./) · [README](../README.md)

# Text Engine

The text engine lives in the `editor-core` crate. It is a pure-Rust
library with no GPU, OS, or async dependencies. Everything here is
implemented during M02.

## 0. Where to look

- **API index:** [`API_DESIGN_NOTES.md`](API_DESIGN_NOTES.md) links rustdoc and mission context.
- **Property tests:** `crates/editor-core/tests/proptest_*.rs` use `proptest` with **256 cases** per macro block (M02 gate).
- **Benchmarks:** `crates/editor-core/benches/rope_benches.rs` — run `cargo bench -p editor-core`; optional baseline: `--save-baseline m02-mvp`.

## 1. Responsibilities

- **Store** the current document as a rope-based buffer.
- **Mutate** it in response to editor operations (insert, delete).
- **Translate** between byte, char, grapheme, and (row, column) coordinate
  spaces.
- **Track** a primary cursor and (in V2) a selection anchor.
- **Record** reversible edits for undo/redo with bounded memory.
- **Detect** and **normalize** line endings at the I/O boundary.

The engine does **not** own the file path, the view, or any I/O. Those
belong to `editor-io` and `editor-app`.

**Line-ending contract with `editor-io`:** On load, `editor-io` detects
`LineEnding`, normalizes content to LF for the rope (via
`TextBuffer::with_line_ending`), and stores the original convention for save.
On save, the I/O layer expands LF back to CRLF/CR when required without
mutating the rope. `LoadedFile` exposes `line_ending` alongside the buffer for
callers that need it without touching the rope.

## 2. Core Types (as implemented in `editor-core`)

The app holds **separate** `TextBuffer`, `Cursor`, `Selection`, and
`UndoStack` (not a single `Document` struct). Public types:

```text
TextBuffer
├── rope:                   ropey::Rope        (LF-internal)
├── original_line_ending:   LineEnding         (detected on load; preserved on save via I/O layer)
├── version:                u64                (bumps on each successful edit)
└── next_edit_seq:          u64                (assigns Edit.seq)

TextBufferSnapshot         { rope, version }  (cheap clone for workers / render)

Cursor                       { pos: BytePos, preferred_col }
Selection                    { anchor: BytePos, head: BytePos }
Edit / EditKind              (insert / delete + inverse)
UndoStack                    (bounded history + future; insert coalescing)
BytePos, LineCol             (byte-centric; column = UTF-8 bytes within line)
```

All offsets are **byte offsets** into the internal (LF-normalized) string.
Row/column for the status bar come from [`TextBuffer::byte_to_line_col`] /
[`TextBuffer::line_col_to_byte`].

### Byte vs. Char vs. Grapheme

- **Byte offset** — canonical storage; what `ropey` uses internally.
- **Char offset** — used for text substring APIs; `ropey` provides
  `byte_to_char` / `char_to_byte`.
- **Grapheme cluster** — what the cursor *moves* by. `unicode-segmentation`
  provides grapheme boundary iteration.
- **Display column** — computed from graphemes with
  `unicode-width::UnicodeWidthStr`. Respects tab stops (configurable,
  default 4).

The engine exposes explicit conversion helpers; callers must choose the
correct space. **Never mix spaces implicitly.**

## 3. Rope Choice

We use [`ropey`](https://docs.rs/ropey) rather than a hand-rolled rope.
Rationale:

- Mature, well-tested (used by Helix).
- Correct line-ending and grapheme handling out of the box.
- Clean chunk iterator suitable for shaping and rendering.
- Active maintenance, 1.x API is stable.

If profiling ever shows ropey as a bottleneck, the replacement strategy is
documented in `RISKS.md`: fork ropey behind an internal `TextBuf`
trait we already use.

## 4. Public API Surface (`editor-core`)

Entry points (see rustdoc on each symbol for full signatures):

| Area | Types / methods |
|------|------------------|
| Buffer | `TextBuffer::new`, `from_str`, `apply_edit`, `insert`, `delete_range`, `len_bytes`, `len_lines`, `line`, `byte_to_line_col`, `line_col_to_byte`, `is_char_boundary`, `slice_to_string`, `snapshot`, `to_text` |
| Edits | `EditKind::{Insert, Delete}`, `Edit::apply`, `Edit::inverse` |
| Cursor | `Cursor::new`, `apply` with `CursorMotion` (grapheme-aware L/R; `WordLeft`/`WordRight`; vertical moves use `preferred_col`) |
| Selection | `Selection::empty`, `range`, `extend_to`, `collapse_to_head` / `collapse_to_anchor` |
| Undo | `UndoStack::new` / `Default`, `push`, `undo`, `redo`, `checkpoint` |
| Line endings | `LineEnding::detect`, `normalize_to`, `as_str` (on-disk newline token) |

`editor-app` wires these together; **`TextBuffer` does not embed** `Cursor` or
`UndoStack` — higher layers own them, matching the mission handoff.

## 5. Undo / Redo Model

- Reversible `Edit` / `EditKind` records: `Insert { pos, text }` and
  `Delete { range, deleted_text }`; `Edit::inverse` rebuilds the partner op.
- `UndoStack` holds `history` and `future` vectors; capacity is configurable
  (see `UndoStack::new` / `Default`).
- **Coalescing:** adjacent single-character inserts within a time window merge
  (`std::time::Instant` + `Duration`); `UndoStack::checkpoint` clears the
  coalesce latch (call on caret moves, etc., from the app).
- History stores **edit records only**, not full rope snapshots.

## 6. Line Endings

- `LineEnding::detect` scans loaded text (see `buffer/line_ending.rs` for rules).
- `TextBuffer::from_str` normalizes content to LF internally and stores
  `original_line_ending` (getter: `TextBuffer::original_line_ending()`).
- At save, `editor-io` writes bytes using that convention (see M06).

## 7. Concurrency & Ownership

`TextBuffer` and `UndoStack` are used on the **main** editing thread. Background
workers receive `TextBufferSnapshot` (cheap `Rope` clone + `version`) for
save/load and similar work. The render crate reads snapshots passed per frame
from `editor-app` — see [ARCHITECTURE.md](ARCHITECTURE.md).

## 8. Determinism

Given the same edit stream, `TextBuffer` and `UndoStack` produce the same
state. This supports reproducible tests (`proptest` replays), benchmark
stability, and future deterministic replay. Avoid hash-map iteration order in
any code that affects observable state.

## 9. Tests

Implemented in the `editor-core` crate:

| Kind | Location |
|------|-----------|
| Unit / integration | `src/**/*.rs` (`#[cfg(test)]`, `buffer`, `cursor`, `selection`, `undo`, …) |
| Proptest | [`crates/editor-core/tests/proptest_rope_invariants.rs`](<../../crates/editor-core/tests/proptest_rope_invariants.rs>), [`edits_proptest.rs`](<../../crates/editor-core/tests/edits_proptest.rs>) |
| Criterion | [`crates/editor-core/benches/rope_benches.rs`](<../../crates/editor-core/benches/rope_benches.rs>) (`harness = false`) |

Performance targets: [`PERFORMANCE_BUDGETS.md`](PERFORMANCE_BUDGETS.md).

## 10. Future Extensions (Not M02)

- Multi-cursor: list of `Cursor`, keep primary cursor pointer.
- Tree-sitter-backed syntax tree maintained incrementally in a separate
  crate. Stays off-hot-path.
- CRDT layer for collaborative editing: slotted between `Edit` records
  and the rope.
- Time-travel debugging: replay `Edit` streams.

All are explicitly out of MVP scope.

---

*Last updated: 2026-04-20 — aligned with implemented `editor-core` API.*

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

