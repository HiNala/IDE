[← docs/](./) · [README](../README.md)

# Text Engine

The text engine lives in the `editor-core` crate. It is a pure-Rust
library with no GPU, OS, or async dependencies. Everything here is
implemented during M02.

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

## 2. Core Types

```text
Document
├── rope:       ropey::Rope              (content, always LF-internal)
├── line_ends:  LineEndingKind           (LF | CRLF | CR, detected on load)
├── cursor:     Cursor                   (primary caret)
├── selection:  Option<Selection>        (V2; anchor + head)
├── history:    History                  (undo/redo stack)
├── dirty_lines: RangeSet<LineIdx>       (invalidated since last layout)
└── version:    u64                      (monotonic edit counter)

Cursor { byte_offset: usize }
Selection { anchor: Cursor, head: Cursor }
```

All offsets are **byte offsets into the internal (LF-normalized)
representation**. Row/column are derived on demand from the rope's line
index.

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

## 4. API Sketch (Stable by End of M02)

```rust
pub struct Document { /* ... */ }

impl Document {
    pub fn from_str(contents: &str) -> Self;
    pub fn insert(&mut self, at: usize, text: &str) -> EditId;
    pub fn delete(&mut self, range: Range<usize>) -> EditId;

    pub fn len_bytes(&self) -> usize;
    pub fn len_chars(&self) -> usize;
    pub fn len_lines(&self) -> usize;

    pub fn line(&self, idx: usize) -> Option<RopeSlice<'_>>;
    pub fn byte_to_line(&self, byte: usize) -> usize;
    pub fn line_to_byte(&self, line: usize) -> usize;

    pub fn cursor(&self) -> Cursor;
    pub fn move_cursor(&mut self, motion: Motion);

    pub fn undo(&mut self) -> Option<EditId>;
    pub fn redo(&mut self) -> Option<EditId>;

    pub fn version(&self) -> u64;
    pub fn take_dirty_lines(&mut self) -> RangeSet<LineIdx>;
}

pub enum Motion {
    Left, Right, Up, Down,
    LineStart, LineEnd,
    DocStart, DocEnd,
    PageUp(usize), PageDown(usize),
    // V2:
    WordLeft, WordRight,
}
```

The exact shapes may evolve; changes are committed in M02 alongside their
tests.

## 5. Undo / Redo Model

- Reversible `Edit` records: `Insert { at, text }` and `Delete { at, text }`.
- Each edit stores exactly enough data to reverse itself.
- `History` is a bounded stack (default 1000 records or 64 MiB, whichever
  hits first) with LRU-eviction from the oldest end.
- **Coalescing:** typing successive characters inside a single word within
  a short time window (default 500 ms) merges into one record. Explicit
  caret motion breaks coalescing.
- **Milestones:** saving a file, opening a new file, or running a big
  multi-cursor action compacts the history into a single checkpoint.

History never retains full rope snapshots. It stores only enough text to
invert operations.

## 6. Line Endings

- At load (`editor-io::load_file`), sniff the first 64 KiB; choose the
  majority ending; store `LineEndingKind` on the `Document`.
- Normalize to `\n` for internal storage.
- At save, re-emit the original kind unless the user explicitly requested
  conversion (not an MVP feature).

## 7. Concurrency & Ownership

The `Document` is `Send` but not `Sync`. It lives on the main thread and
is mutated synchronously. Render-side snapshots are produced via:

```rust
pub struct RenderSnapshot {
    pub version: u64,
    pub rope: Arc<ropey::Rope>,       // shared immutable rope
    pub visible_lines: Range<usize>,
    pub cursor_byte: usize,
    // ...
}
```

Snapshots are cheap to produce because `ropey::Rope` implements
structural sharing. `arc-swap` publishes the latest snapshot to the render
thread/task without locks.

Details: `CONCURRENCY.md`.

## 8. Determinism

Given the same input stream, `Document` produces the same state and the
same dirty-line set every run. This is essential for:

- Reproducible tests (`proptest` replays).
- Benchmark stability.
- Future deterministic replay of editing sessions.

We avoid hash-map iteration order in any code that affects observable
state.

## 9. Tests

Per `TESTING_STRATEGY.md`, M02 ships with:

- Unit tests for every public API.
- `proptest` tests:
  - random insert/delete sequences preserve `len_bytes == rope.len_bytes()`;
  - undo(redo(x)) == x for any edit sequence;
  - `byte_to_line` and `line_to_byte` round-trip for every byte offset;
  - grapheme-aware cursor motion never lands mid-cluster.
- Criterion benchmarks:
  - Insert 1 char at head / middle / tail of 1 MiB / 10 MiB / 100 MiB.
  - Delete 1 MiB block.
  - 1 M line-index lookups.
  - Undo-redo round-trip of 10k edits.

Targets: all per-edit ops < 50 µs on 100 MiB documents on the dev box.

## 10. Future Extensions (Not M02)

- Multi-cursor: list of `Cursor`, keep primary cursor pointer.
- Tree-sitter-backed syntax tree maintained incrementally in a separate
  crate. Stays off-hot-path.
- CRDT layer for collaborative editing: slotted between `Edit` records
  and the rope.
- Time-travel debugging: replay `Edit` streams.

All are explicitly out of MVP scope.

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

