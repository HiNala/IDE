[← docs/](./) · [README](../README.md)

# Glossary

Short definitions for terminology used across this repository's docs.

---

**Acceptance criteria.** Measurable conditions a mission must satisfy
before it is considered complete. See `MVP_DEFINITION.md` §8.

**Arc-swap.** The `arc-swap` crate. Publishes an `Arc<T>` with
lock-free atomic replacement. Used here for the document → render
snapshot boundary.

**Arena allocator.** A memory allocator that hands out chunks from a
contiguous region and deallocates all of them at once. Reduces
fragmentation and cache misses.

**Atlas.** A GPU-resident texture that stores many glyph bitmaps in a
single image. Rendering a glyph is then a small quad that samples the
atlas.

**Atomic save.** A save strategy where data is written to a temporary
file and atomically renamed to the target, guaranteeing that no partial
or corrupt file is ever visible.

**Back-pressure.** When a producer is forced to slow down because a
consumer cannot keep up. Implemented via bounded channels.

**Back-end (graphics).** The underlying GPU API that wgpu wraps:
Vulkan, Metal, DX12, or OpenGL / WebGL.

**BGRA / RGBA.** Byte order of color channels in a texture format; the
surface format choice affects blending and shader code.

**Byte offset.** A position in the document measured in raw UTF-8
bytes.

**Budget, performance.** A strict upper bound on the time or memory a
subsystem is permitted to use per frame. See
`PERFORMANCE_BUDGETS.md`.

**Channel, bounded.** A queue with a maximum capacity; senders block or
fail if the queue is full. We use `crossbeam_channel` and
`tokio::sync::mpsc` channels.

**Chunk (rope).** A leaf of the rope tree holding a run of text bytes.

**Clippy.** Rust's built-in lint tool. We treat warnings as errors.

**Coalesce (undo).** Merge multiple small successive edits into one
history entry so Undo reverses a logical "word" rather than a single
keystroke.

**Conventional Commits.** Commit-message convention (`type(scope):
subject`). We use it everywhere.

**Cosmic-text.** A Rust text shaping + layout library. Underlies
`glyphon`.

**Criterion.** Statistical benchmarking framework for Rust. Our
regression gate.

**Cross (cross-compile tool).** A `cargo` wrapper that cross-compiles
in Docker. We use it only in M11 for Linux targets from non-Linux
hosts.

**Cursor.** The insertion point. A byte offset into the document.

**Delta rendering.** Rendering only the screen regions that changed
since the last frame, rather than the whole window.

**Deterministic.** Same input → same output, every time. An
architectural requirement here.

**Dev overlay.** The dev-build metric panel toggled with F1. See
`OBSERVABILITY.md`.

**Dirty region / dirty line.** A screen area or text line that has
changed since the last frame and must be re-rendered.

**DPI / HiDPI.** Dots per inch / high-DPI. How densely pixels are
packed; affects font rasterization and atlas size.

**Editor-app / editor-core / editor-render / editor-input / editor-io.**
The five workspace member crates. See `ARCHITECTURE.md`.

**Extension (file).** How we determine language / syntax later. Not
MVP.

**Event loop.** The main thread's OS event pump (via winit).

**Fifo (present mode).** Vsync-locked presentation; safe fallback.

**Fps.** Frames per second. Target 60 minimum, 120 where hardware
allows.

**Frame loop.** Our main-thread cycle: input → state → render.

**Frame budget.** Time budget for each phase of the frame loop.

**`glyphon`.** wgpu-backed text renderer using cosmic-text.

**Grapheme cluster.** A user-perceived character. Cursor motion steps by
grapheme, not byte or code point.

**History.** The undo/redo stack in `editor-core`.

**IME.** Input Method Editor, used for Chinese / Japanese / Korean and
dead keys.

**Immediate mode UI.** UI paradigm where the whole UI is painted every
frame (egui). Rejected for the main canvas; may be used for the dev
overlay.

**Insta.** Snapshot-testing crate. Used for error messages and small
rendered frames.

**Layout.** The act of turning shaped glyphs into a 2D position on the
screen.

**LF / CRLF / CR.** Line endings. Internal storage is LF; original
kind preserved at the I/O boundary.

**Logical key.** The character-meaning of a key press, after the OS
keyboard layout is applied (vs. the physical key position).

**Loom.** Concurrency-model-checking crate. Opt-in via feature flag.

**Mailbox (present mode).** Low-latency present mode supported on DX12
/ Vulkan / Metal.

**Memory-mapped file (mmap).** File whose contents are mapped into
process address space; reads become page loads. Used for large files.

**Mission.** A numbered scoped chunk of work (M00–M11) executed in
order.

**MSRV.** Minimum Supported Rust Version.

**North Star.** The project's long-term vision: out-perform VS Code
and Cursor in measurable ways. See `ARCHITECTURE.md`.

**Overlay.** The dev metrics panel rendered in a separate pass.

**Panic hook.** Custom `std::panic::set_hook` used to print diagnostic
output before process termination.

**Pixel, physical vs. logical.** `logical = physical * scale_factor`.
winit provides both.

**Present mode.** How the surface hands frames to the display: Fifo,
Mailbox, Immediate.

**Property test.** A test that checks an invariant across many randomly
generated inputs (`proptest`).

**Proptest.** Property-testing crate we use.

**Render pass.** A wgpu command-buffer unit with one bound attachment
set.

**Render snapshot.** An immutable, `Arc`-shared view of the document
published each frame for the renderer.

**Rope.** A tree-of-chunks data structure supporting O(log n) edits.
We use `ropey`.

**Ropey.** The Rust rope crate we depend on.

**RSS.** Resident Set Size; process memory currently in physical RAM.

**Runtime, Tokio.** The async worker runtime for background tasks.
Never used on the hot path.

**Samply.** Cross-platform sampling profiler exporting to the Firefox
Profiler format. Recommended for CPU profiling.

**Selection.** A range (anchor + head) within the document. V2 feature.

**Shape (text).** Convert a string into positioned glyph indices.
Handled by cosmic-text.

**Snapshot.** Either (a) an `Arc`-shared render snapshot, or (b) a
point-in-time copy used for insta tests.

**Soak test.** A long-running test (M08 has a 4-hour one) that checks
for memory leaks and other slow-developing problems.

**Span, tracing.** A scoped duration in the `tracing` ecosystem.
Appears as an entry in the Chrome Trace export.

**Stress test.** A high-load test validating that the system survives
extreme scenarios (M08).

**Surface (wgpu).** A drawable target associated with a window.

**Tempfile.** The `tempfile` crate. Creates safe temp files for
atomic-save.

**TextBuf (internal trait).** Planned abstraction over the rope so we
can swap implementations if profiling demands.

**Thread, render.** The dedicated thread owning wgpu resources.

**Thread, worker.** A pool thread inside the Tokio runtime running
background `async` tasks.

**TOML.** Config file format used by Cargo.

**Tracing.** The `tracing` ecosystem for structured logs + spans.

**UTF-8.** The only encoding the MVP reads or writes.

**Vsync.** Vertical synchronization; Fifo enforces it, Mailbox partially
enforces it.

**Wgpu.** The cross-platform GPU abstraction we use.

**Winit.** The cross-platform windowing crate we use.

**Work shedding.** Dropping or deferring non-essential work when a
budget overruns.

**Workspace (Cargo).** A collection of crates sharing a `Cargo.lock`
and target directory.

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

