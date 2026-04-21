# M02 — Text Engine: Rope Buffer, Cursor, Selection, Undo/Redo

**Mission ID:** M02
**Prerequisites:** M01 complete (workspace scaffolded, CI green, `editor-core` has error types).
**Output:** A fully functional text engine in `editor-core` with rope-based storage, cursor and selection types, an operation-log undo/redo system, exhaustive unit tests, property-based tests, and Criterion benchmarks.
**Estimated scope:** 2-3 sessions (a lot of careful logic and testing).

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/docs/TEXT_ENGINE.md` — the reference for rope choice, cursor primitives, undo/redo, line endings.
- `/docs/RUST_CONVENTIONS.md` — error handling and naming conventions.
- `/docs/TESTING_STRATEGY.md` — the testing pyramid and property-based test approach.
- `/reference/01_TECH_STACK.md` and `/reference/02_ARCHITECTURE_STRATEGY.md` — the PRD's take on the text engine.
- `https://docs.rs/ropey/latest/ropey/` — Ropey's API surface. Read this carefully. Understand `Rope`, `RopeSlice`, `char_to_byte`, `byte_to_char`, `line_to_char`, `char_to_line`, and the chunks iterator.
- `https://zed.dev/blog/zed-decoded-rope-sumtree` — conceptual background on why ropes matter for editors and why Zed went further.

---

## The Situation In Plain English

The text engine is the heart of the editor. Everything else — rendering, input, file I/O — talks to it or depends on its behavior. If the text engine is wrong, slow, or fragile, nothing else can save the editor. This mission exists to get it right the first time.

We chose Ropey as the MVP rope implementation in M00/M01 because it's the most widely deployed, best-tested, and Unicode-correct rope crate in the Rust ecosystem. Ropey handles UTF-8 safely, tracks line breaks (including CRLF correctly), supports cheap `Arc`-backed cloning for thread-safe snapshots, and gives us O(log N) inserts and deletes even on gigabyte-sized documents. Those properties are exactly what we need. Future missions may migrate to a SumTree-style rope (à la Zed) for even better performance at scale, but we're not solving that problem now — we're establishing correctness and a clean abstraction.

Around the rope we build the primitives every editor needs: a `Cursor` (a single insertion point with byte-offset precision), a `Selection` (an anchor and a head, with the cursor being the head), an `Edit` (a pure data description of a text change), and an `UndoStack` (an operation log of edits and their inverses). We also build a line-ending detector and writer so we can preserve the user's original convention on save.

The crucial discipline in this mission is **no I/O, no UI, no GPU**. `editor-core` is pure. It knows about bytes, chars, lines, edits, and time (for undo coalescing). It does not know about files, windows, or rendering. This boundary is what lets us unit-test everything to high coverage and lets downstream crates move quickly.

Performance matters even here. We target: 1M random insertions per second on a multi-MB rope, sub-microsecond cursor-move operations, and sub-millisecond undo/redo application for typical edits. Criterion benchmarks prove these numbers; property-based tests (proptest) prove correctness under adversarial sequences of edits.

---

## Scope

**In scope:**
- `TextBuffer` — a wrapper around `ropey::Rope` with a clean API.
- `Cursor` and `Selection` primitives.
- `Edit` struct (insert/delete operations).
- `UndoStack` with inverse operations and rapid-typing coalescing.
- Line ending detection (LF, CRLF, CR, mixed) and normalization.
- Public API exported from `editor-core`.
- Unit tests for every public function.
- Property-based tests (proptest) for rope invariants.
- Criterion benchmarks for hot paths.
- Documentation (rustdoc) for every public item.

**Out of scope:**
- File I/O (opens/saves) — that's M06.
- Multiple cursors / multi-selection — post-V2.
- Syntax trees — post-V2.
- Memory-mapped rope storage for huge files — post-V2 optimization.

---

## North Star For This Mission

A developer can, from `editor-app` or from a test, construct a `TextBuffer`, insert text, move a cursor around, make a selection, perform edits, undo them, and redo them — all with sub-millisecond latency even on documents tens of megabytes large, and with proptest-verified correctness.

---

## TODO List

### 1. Design the public API of `editor-core` before writing implementation

- [ ] 1.1. Open `/docs/TEXT_ENGINE.md`. On paper (or in a new file `docs/API_DESIGN_NOTES.md`), sketch the public API: what types we expose, what methods they have, what error types they return. Resist the temptation to expose `ropey::Rope` directly — we want the option to swap implementations later, and raw Ropey leaks types like `RopeSlice` and `char indices` that are wrong for our byte-centric design.
- [ ] 1.2. Decide byte offsets are canonical. Everything takes `usize` byte offsets. Line and column are derived. (Rust-style: matches `String` indexing, avoids UTF-16 confusion.) Explicitly document that char indices are not part of our public surface.
- [ ] 1.3. Decide on module layout:
  ```
  crates/editor-core/src/
  ├── lib.rs                  (public re-exports, doc comment)
  ├── error.rs                (CoreError, CoreResult — existing from M01)
  ├── buffer/
  │   ├── mod.rs              (TextBuffer struct, public API)
  │   ├── edit.rs             (Edit, EditKind, apply logic)
  │   └── line_ending.rs      (LineEnding enum, detect, normalize)
  ├── cursor.rs               (Cursor, CursorMotion)
  ├── selection.rs            (Selection, anchor/head semantics)
  ├── undo.rs                 (UndoStack, snapshot coalescing)
  └── position.rs             (Position, BytePos, LineCol converters)
  ```
- [ ] 1.4. Commit: `docs(core): sketch editor-core API surface`.

### 2. Implement `Position`, `BytePos`, `LineCol` newtypes

- [ ] 2.1. `crates/editor-core/src/position.rs`:
  ```rust
  //! Position types for the text buffer.
  //!
  //! Byte offsets are canonical. `LineCol` exists for human-facing display
  //! (e.g., the status bar) but is derived, not stored.

  /// A byte offset into the buffer's UTF-8 content. Guaranteed to lie on
  /// a UTF-8 code point boundary when produced by this crate's APIs.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
  pub struct BytePos(pub usize);

  /// A (line, column) pair. Column is measured in UTF-8 bytes within the line,
  /// not in grapheme clusters or display width. For display purposes, convert
  /// through a grapheme segmenter.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
  pub struct LineCol {
      pub line: usize,
      pub col: usize,
  }
  ```
- [ ] 2.2. Implement `From<usize> for BytePos`, `Into<usize> for BytePos`, and arithmetic (`Add`, `Sub`) where it makes sense.
- [ ] 2.3. Document: BytePos should never land mid-codepoint. If callers construct one from a raw byte index that's not a boundary, they get a `CoreError::InvalidOffset` when they try to use it with the buffer.
- [ ] 2.4. Unit tests: construction, comparison, arithmetic, doctests.
- [ ] 2.5. Commit: `feat(core): add BytePos and LineCol position types`.

### 3. Implement `LineEnding`

- [ ] 3.1. `crates/editor-core/src/buffer/line_ending.rs`:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum LineEnding {
      Lf,      // "\n" — Unix/modern
      Crlf,    // "\r\n" — Windows
      Cr,      // "\r" — classic Mac (very rare today)
      Mixed,   // file contains more than one convention
  }

  impl LineEnding {
      pub fn detect(text: &str) -> LineEnding { /* ... */ }
      pub fn as_str(self) -> &'static str { /* ... */ }
      /// Convert all line endings in the given string to the target.
      pub fn normalize_to(target: LineEnding, input: &str) -> String { /* ... */ }
  }
  ```
- [ ] 3.2. `detect` scans the first ~64 KB (or the whole file if smaller) and returns the first convention seen, unless it sees a second different one, in which case `Mixed`.
- [ ] 3.3. Unit tests: empty string, pure LF, pure CRLF, pure CR, mixed, files with no line endings.
- [ ] 3.4. Commit: `feat(core): add LineEnding detection and normalization`.

### 4. Implement `TextBuffer` wrapping `ropey::Rope`

- [ ] 4.1. `crates/editor-core/src/buffer/mod.rs`:
  ```rust
  use ropey::Rope;

  pub struct TextBuffer {
      rope: Rope,
      /// Original line ending on load. Saves preserve this by default.
      original_line_ending: LineEnding,
      /// Incremented on every edit. External systems can compare to detect changes.
      version: u64,
  }
  ```
- [ ] 4.2. Constructors: `TextBuffer::new()` (empty), `TextBuffer::from_str(s: &str)` (auto-detects line ending, normalizes to LF internally), `TextBuffer::with_line_ending(s: &str, le: LineEnding)`.
- [ ] 4.3. Read accessors: `len_bytes(&self) -> usize`, `len_lines(&self) -> usize`, `line(&self, line: usize) -> Option<RopeLineSlice>` where `RopeLineSlice` is a small wrapper around `RopeSlice` that exposes `as_str` and `len_bytes` but nothing else.
- [ ] 4.4. Position conversion: `byte_to_line_col(&self, pos: BytePos) -> CoreResult<LineCol>`, `line_col_to_byte(&self, lc: LineCol) -> CoreResult<BytePos>`. These are the only way downstream crates get line/col.
- [ ] 4.5. Boundary validation: `is_char_boundary(&self, pos: BytePos) -> bool`. Use Ropey's own boundary check.
- [ ] 4.6. Slicing: `slice_to_string(&self, range: Range<BytePos>) -> CoreResult<String>` for when rendering or clipboard needs a range as a `String`.
- [ ] 4.7. Raw write access: `insert(&mut self, pos: BytePos, text: &str) -> CoreResult<()>`, `delete(&mut self, range: Range<BytePos>) -> CoreResult<String>` (returns the deleted text so callers can build inverse edits). Both bump `version`.
- [ ] 4.8. Iteration: expose a `chunks` iterator returning `(BytePos, &str)` tuples. This is what the renderer uses to render visible lines without copying.
- [ ] 4.9. Cheap snapshots: `snapshot(&self) -> TextBufferSnapshot` where `TextBufferSnapshot` holds a clone of the `Rope` (Arc-backed, cheap) and the version number. Used by background tasks (future file save in M06, future Tree-sitter parse).
- [ ] 4.10. Unit tests for every public function: insert at start/middle/end, delete empty/single-char/multi-line range, position conversion for ASCII/UTF-8/emoji/CJK, boundary validation catches mid-codepoint offsets, version bumps.
- [ ] 4.11. Doctests on every public function showing intended use.
- [ ] 4.12. Commit: `feat(core): implement TextBuffer wrapping Ropey with byte-centric API`.

### 5. Implement `Cursor` and `CursorMotion`

- [ ] 5.1. `crates/editor-core/src/cursor.rs`:
  ```rust
  pub struct Cursor {
      /// Current byte position in the buffer.
      pos: BytePos,
      /// Desired column for vertical moves; preserved across up/down so the
      /// cursor "remembers" its original column when moving through shorter lines.
      preferred_col: Option<usize>,
  }

  pub enum CursorMotion {
      Left,
      Right,
      Up,
      Down,
      LineStart,
      LineEnd,
      BufferStart,
      BufferEnd,
      ByteOffset(BytePos),
  }
  ```
- [ ] 5.2. `Cursor::apply(&mut self, motion: CursorMotion, buffer: &TextBuffer) -> CoreResult<()>`. Pure logic; no I/O.
- [ ] 5.3. `Left` and `Right` must move by grapheme cluster, not by byte or codepoint. Use the `unicode-segmentation` crate for this (add to `editor-core`'s deps if not already there). Document why: emoji like "👨‍👩‍👧‍👦" are multiple codepoints but one grapheme — the cursor should move past the whole cluster.
- [ ] 5.4. `Up` and `Down` respect `preferred_col`: when moving vertically, target the `preferred_col` in the new line, or the line's end if it's shorter. When moving horizontally, update `preferred_col`. When the cursor crosses into a new line via vertical motion, preserve the `preferred_col` for the next vertical motion.
- [ ] 5.5. Boundary behavior: `Up` at the top line stays at the top (or jumps to `BufferStart`, document which — pick the VS Code convention: stays put at top line, optionally moves to column 0 on some keybindings handled one level up). `Down` at the bottom similarly.
- [ ] 5.6. Unit tests: every motion on a variety of buffers. Property test: any sequence of motions leaves the cursor on a valid char boundary.
- [ ] 5.7. Commit: `feat(core): implement Cursor with grapheme-aware motion and preferred column`.

### 6. Implement `Selection`

- [ ] 6.1. `crates/editor-core/src/selection.rs`:
  ```rust
  /// A text selection. Anchor is where the selection started; head is where the
  /// cursor currently is. Head can be before or after anchor.
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct Selection {
      pub anchor: BytePos,
      pub head: BytePos,
  }

  impl Selection {
      pub fn empty(pos: BytePos) -> Self { Selection { anchor: pos, head: pos } }
      pub fn is_empty(self) -> bool { self.anchor == self.head }
      pub fn range(self) -> Range<BytePos> { /* min..max */ }
      pub fn len_bytes(self) -> usize { /* max - min */ }
  }
  ```
- [ ] 6.2. Selection modification: `extend_to(&mut self, new_head: BytePos)`, `collapse_to_head(&mut self)`, `collapse_to_anchor(&mut self)`.
- [ ] 6.3. Unit tests: empty selections, selections crossing each other (anchor > head), range computation, length in bytes.
- [ ] 6.4. Commit: `feat(core): add Selection primitive with anchor/head semantics`.

### 7. Implement `Edit`

- [ ] 7.1. `crates/editor-core/src/buffer/edit.rs`:
  ```rust
  #[derive(Debug, Clone)]
  pub enum EditKind {
      Insert { pos: BytePos, text: String },
      Delete { range: Range<BytePos>, deleted_text: String }, // deleted_text stored for undo
  }

  #[derive(Debug, Clone)]
  pub struct Edit {
      pub kind: EditKind,
      /// Monotonically increasing sequence number assigned by TextBuffer.
      pub seq: u64,
  }

  impl Edit {
      /// Apply this edit to a buffer. Returns the edit itself (useful for the undo stack)
      /// or an error if positions are invalid.
      pub fn apply(&self, buffer: &mut TextBuffer) -> CoreResult<()> { /* ... */ }

      /// Produce the inverse edit that would undo this one.
      pub fn inverse(&self) -> Edit { /* ... */ }
  }
  ```
- [ ] 7.2. `TextBuffer` gains an `apply_edit(&mut self, kind: EditKind) -> CoreResult<Edit>` method that constructs an `Edit`, applies it, and returns it (so callers can push it onto the undo stack).
- [ ] 7.3. `Edit::inverse` for `Insert { pos, text }` returns `Delete { range: pos..(pos + text.len()), deleted_text: text.clone() }`. For `Delete { range, deleted_text }` returns `Insert { pos: range.start, text: deleted_text.clone() }`.
- [ ] 7.4. Unit tests: apply+inverse round-trip returns the buffer to its starting state. Property test: random edit sequences followed by their inverses (in reverse order) restore the original buffer byte-for-byte.
- [ ] 7.5. Commit: `feat(core): add Edit and EditKind with apply/inverse`.

### 8. Implement `UndoStack` with coalescing

- [ ] 8.1. `crates/editor-core/src/undo.rs`:
  ```rust
  pub struct UndoStack {
      /// Edits pushed since the last undo. When the user undoes, we pop from
      /// `history`, apply the inverse, and push the popped edit onto `future`.
      history: Vec<Edit>,
      future: Vec<Edit>,
      /// Time of the last push. Used for coalescing rapid typing.
      last_push: Option<Instant>,
      /// Maximum edits to retain. Oldest evicted when exceeded.
      capacity: usize,
      /// Window within which contiguous single-character inserts at adjacent
      /// positions coalesce into one edit.
      coalesce_window: Duration,
  }
  ```
- [ ] 8.2. `UndoStack::push(&mut self, edit: Edit)`: clears `future`, coalesces if the previous edit is a single-char insert adjacent in position and within `coalesce_window`, else pushes as-is.
- [ ] 8.3. `UndoStack::undo(&mut self, buffer: &mut TextBuffer) -> CoreResult<Option<Edit>>`: pops from `history`, applies inverse to buffer, pushes onto `future`.
- [ ] 8.4. `UndoStack::redo(&mut self, buffer: &mut TextBuffer) -> CoreResult<Option<Edit>>`: pops from `future`, re-applies original to buffer, pushes onto `history`. Does not coalesce.
- [ ] 8.5. `UndoStack::checkpoint(&mut self)`: force a coalescing boundary. Called on events like cursor moves, file saves, focus changes (handled in M09 by the UI layer).
- [ ] 8.6. Unit tests: single insert + undo, multiple inserts + undo, rapid typing coalesces, non-adjacent typing doesn't coalesce, undo + new edit clears future, capacity eviction.
- [ ] 8.7. Property test: any sequence of pushes followed by N undos followed by N redos returns the buffer to its state right after the Nth push. Use `proptest` for this.
- [ ] 8.8. Commit: `feat(core): implement UndoStack with coalescing and time-based boundaries`.

### 9. Wire everything together via `lib.rs`

- [ ] 9.1. `crates/editor-core/src/lib.rs`:
  ```rust
  //! editor-core: the pure text-engine crate.
  //!
  //! This crate owns the text buffer, cursor, selection, edit, and undo-stack
  //! primitives. It has no OS, GPU, or I/O dependencies. Everything here must be
  //! unit-testable and deterministic.

  #![warn(clippy::all, clippy::pedantic)]
  #![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

  mod buffer;
  mod cursor;
  mod error;
  mod position;
  mod selection;
  mod undo;

  pub use buffer::{LineEnding, TextBuffer, TextBufferSnapshot};
  pub use buffer::edit::{Edit, EditKind};
  pub use cursor::{Cursor, CursorMotion};
  pub use error::{CoreError, CoreResult};
  pub use position::{BytePos, LineCol};
  pub use selection::Selection;
  pub use undo::UndoStack;
  ```
- [ ] 9.2. Add crate-level doctest showing a minimal editing session: create buffer, apply edits, undo, redo.
- [ ] 9.3. Commit: `feat(core): export public API via lib.rs`.

### 10. Write property-based tests

- [ ] 10.1. Create `crates/editor-core/tests/proptest_rope_invariants.rs`. Use `proptest` (already in dev-deps from M01).
- [ ] 10.2. Test: for any sequence of valid inserts and deletes applied to an empty buffer, the buffer's byte count matches the sum of insertions minus the sum of deletion lengths.
- [ ] 10.3. Test: for any sequence of edits followed by their inverses in reverse order, the buffer returns to empty (or the original state).
- [ ] 10.4. Test: `byte_to_line_col` and `line_col_to_byte` are inverse operations for any valid input.
- [ ] 10.5. Test: `Cursor` motion keeps the cursor on a valid char boundary under any sequence of motions on any buffer.
- [ ] 10.6. Test: `UndoStack` round-trip property — N pushes, N undos, N redos → state matches state after N pushes.
- [ ] 10.7. Commit: `test(core): add proptest-based rope and undo-stack invariants`.

### 11. Write Criterion benchmarks

- [ ] 11.1. Create `crates/editor-core/benches/rope_benches.rs`. Benchmarks:
  - `bench_insert_random_coherent` — insert 10,000 single chars at near-same position in a 10MB buffer. Expect microseconds per insert.
  - `bench_insert_random_incoherent` — insert 10,000 single chars at random positions in a 10MB buffer.
  - `bench_delete_random_ranges` — delete 10,000 random ranges of length 1..100.
  - `bench_line_iteration` — iterate every line of a 10MB buffer.
  - `bench_byte_to_line_col` — 10,000 random conversions.
  - `bench_cursor_motion_up_down` — 10,000 up/down cursor moves across a 10MB buffer.
  - `bench_undo_coalescing` — push 10,000 single-char edits and measure coalescing overhead.
  - `bench_snapshot_clone` — clone the buffer 10,000 times.
- [ ] 11.2. Set `harness = false` in `Cargo.toml` (already done in M01).
- [ ] 11.3. Run `cargo bench -p editor-core` locally and save results. Compare against rough targets from `/docs/PERFORMANCE_BUDGETS.md`. If anything is wildly slow (> 10× target), investigate.
- [ ] 11.4. Use `black_box` on all inputs to prevent dead-code elimination.
- [ ] 11.5. Store Criterion baseline under a named baseline: `cargo bench -- --save-baseline m02-mvp`.
- [ ] 11.6. Commit: `bench(core): add Criterion benchmarks for rope and cursor hot paths`.

### 12. Handle edge cases and polish

- [ ] 12.1. Empty buffer: every operation should work on an empty buffer without panicking. `cursor.apply(Right, &buf)` when buf is empty stays at position 0.
- [ ] 12.2. Single-line buffer without trailing newline: line count is 1, not 0. `line(0)` returns the whole content.
- [ ] 12.3. Buffer with trailing newline: line count is N where N lines of content + 1 empty line after.
- [ ] 12.4. Very large insert: insert a 1MB string into a 10MB buffer. Should work without panic.
- [ ] 12.5. UTF-8 edge cases: the byte after `\u{1F600}` (😀, 4 bytes) must be treated as a valid boundary; mid-codepoint must not. Add explicit tests.
- [ ] 12.6. Grapheme cluster edge cases: family emoji like 👨‍👩‍👧‍👦, flag emoji like 🇺🇸, combining marks like é (as e + ́).
- [ ] 12.7. CRLF line-ending bugs: ensure `line_to_char` plus `char_to_byte` correctly map lines when the file uses CRLF.
- [ ] 12.8. Commit: `fix(core): address UTF-8, grapheme, and line-ending edge cases with tests`.

### 13. Documentation

- [ ] 13.1. Every public item has a `///` rustdoc comment. Every module has a `//!`. Run `cargo doc --open` and read your own docs.
- [ ] 13.2. Update `/docs/TEXT_ENGINE.md` with the actual API and any decisions you made that differ from the M00 draft. Note any open questions in `/FOLLOWUPS.md`.
- [ ] 13.3. Add a code example to the top of `lib.rs`'s `//!` block showing a complete editing session (create, insert, move cursor, select, delete, undo, redo).
- [ ] 13.4. Commit: `docs(core): complete rustdoc for public API`.

### 14. Quality gates and self-review

- [ ] 14.1. `cargo fmt --all -- --check`.
- [ ] 14.2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`. If clippy flags something in proptest-generated code, allow it locally with a `#[allow]` comment explaining why, not globally.
- [ ] 14.3. `cargo test --workspace --all-features` — every test passes on Windows.
- [ ] 14.4. `cargo bench -p editor-core` — all benchmarks complete; compare against baseline.
- [ ] 14.5. `cargo doc --workspace --no-deps` — no broken links.
- [ ] 14.6. Push. Confirm CI green.

### 15. Update project docs

- [ ] 15.1. Update `/docs/STATUS.md`: M02 complete, M03 next.
- [ ] 15.2. Update `/CHANGELOG.md` under `[Unreleased]`:
  ```
  ### Added
  - `editor-core::TextBuffer` rope-based text buffer with byte-centric API.
  - `Cursor` with grapheme-aware motion and preferred-column tracking.
  - `Selection` primitive.
  - `Edit`/`EditKind` with apply/inverse semantics.
  - `UndoStack` with rapid-typing coalescing and bounded capacity.
  - `LineEnding` detection and normalization.
  - Criterion benchmarks for all hot paths.
  - Proptest-based invariant tests.
  ```
- [ ] 15.3. Tag: `git tag -a m02-complete -m "M02 complete: text engine in editor-core"`; push tag.

---

## Validation / Acceptance Criteria

M02 is complete when:

1. `cargo test -p editor-core --all-features` passes on Windows, Linux, and macOS.
2. `cargo bench -p editor-core` completes; results captured as the `m02-mvp` baseline.
3. `cargo clippy -p editor-core --all-targets --all-features -- -D warnings` is clean.
4. `cargo doc -p editor-core --no-deps` builds with zero warnings.
5. Proptest suite runs at least 256 random cases per property without failure.
6. Every public item has rustdoc, including a doctest where appropriate.
7. A `lib.rs` crate-level doctest demonstrates a complete edit session.
8. `docs/TEXT_ENGINE.md` is updated to match the implemented API.
9. `docs/STATUS.md` reflects "M02 done, M03 next."
10. `m02-complete` tag is pushed.

## Testing Requirements

- Unit tests on every public function in every module.
- Doctests on every public item.
- Property-based tests (proptest) for rope invariants, position conversions, cursor motion, and undo/redo round-trips.
- Criterion benchmarks on hot paths with captured baselines.
- Tests run green on all three OSes in CI.

## Git Commit Strategy

Expect 14-18 commits. Push after each major section (items 4, 7, 8, 10, 11, 13, 14).

## Handoff to M03

M03 assumes:

- `editor-core` exports `TextBuffer`, `Cursor`, `Selection`, `Edit`, `UndoStack`.
- These types are ready to be owned and mutated by higher layers (the app).
- Benchmarks exist to catch regressions caused by downstream changes.

M03 will add real windowing + GPU context (beyond the smoke test) so M04 can render text.

---

## Standing Orders Reminder

- No panics in the public API. Every fallible operation returns `CoreResult`.
- No `unsafe` in `editor-core`. If you need it, stop and escalate.
- If a proptest case fails intermittently, do not mark the test `#[ignore]`. Investigate. Flaky tests lie. A failing proptest means the implementation is wrong.
- Criterion baselines are saved only on `main`. Don't save baselines from a feature branch (confusing for later).
- If Ropey's API has changed since these notes were written, adapt. Update `/docs/TEXT_ENGINE.md` accordingly.

---

## As-built verification (2026-04)

The `editor-core` crate on `main` **meets M02’s functional bar**: `TextBuffer` (rope, LF-internal, `LineEnding`), `BytePos`/`LineCol`, `Cursor` + `CursorMotion` (grapheme-aware via `unicode-segmentation`), `Selection`, `Edit`/`EditKind` with `apply`/`inverse`, `UndoStack` with time-window coalescing and `checkpoint`, unit tests, `crates/editor-core/tests/*.rs` (proptest at **256 cases** per `proptest!` block, plus cursor-boundary and undo/redo round-trip properties), and `crates/editor-core/benches/rope_benches.rs` (includes incoherent insert, random deletes, cursor up/down, undo push+checkpoint; very large buffers scaled down for laptop-friendly runs). `docs/TEXT_ENGINE.md` and [`API_DESIGN_NOTES.md`](../API_DESIGN_NOTES.md) describe the **actual** public API.

**Doc / process deltas vs this mission file:** Saving a named baseline (`--save-baseline m02-mvp`) is optional and local. Tags like `m02-complete` are historical; use git tags if you need an audit trail.

Go.
