[← docs/](./) · [README](../README.md)

# Product Requirements Document (MVP)

This PRD is the in-repo, canonical product requirements document for the MVP.
It restates and condenses the original product brief so that agents do not
need to go hunting for source material. If this document drifts from the
brief, this document wins for implementation purposes; open an issue if the
drift is material.

## 1. Vision

A high-performance, native code editor designed from first principles to
eliminate the latency and bloat of Electron-based tools like Visual Studio
Code and Cursor. The MVP is not a full IDE; it is a proof that a
performance-first architecture can deliver an editing experience that feels
**instantaneous** regardless of file size or system load.

Success at the MVP stage is measured by:

- Perceptible speed improvements over VS Code and Cursor under identical
  conditions.
- Consistent, predictable behavior across machines with different hardware.

## 2. Philosophy and Constraints

- **Performance-first.** Every architectural decision must justify its cost
  in latency, memory, or CPU.
- **Sub-5 ms input-to-pixel latency** under normal conditions.
- **Never block the real-time loop** for background work.
- **Bounded, predictable memory** — no fragmentation patterns, no unbounded
  growth over time.
- **Deterministic behavior** — identical inputs produce identical state
  transitions and pixels.
- **No unnecessary abstraction** — no frameworks, no VMs, no embedded
  browsers.

## 3. Target Environment

- Windows 10 / 11, macOS (Apple Silicon and Intel), modern Linux
  distributions (Wayland and X11).
- Architectures: `x86_64` and `aarch64` where feasible.
- Hardware range: low-power laptops through high-end desktops.
- GPU: GPU acceleration is preferred; a fallback rendering path exists for
  systems without suitable GPUs (software-rasterized via wgpu's GL
  backend).

At startup, the system detects available CPU cores, memory, and GPU
capabilities and configures itself accordingly (thread pool size, atlas
size, present mode).

## 4. System Architecture (Summary)

Four layers, strictly separated:

- **Real-time path:** input → text engine → rendering.
- **Background path:** file I/O, indexing, future LSP/AI.
- **System layer:** hardware detection, scheduling, observability.
- **Shell:** the binary crate that composes everything.

Events flow through bounded channels. The real-time path never waits on the
background path.

See `ARCHITECTURE.md` and `CONCURRENCY.md` (both in this directory) for
full detail.

## 5. Text Engine

- **Rope-based document** (via `ropey`) for O(log n) edits.
- **Grapheme-aware cursor and selection** (via `unicode-segmentation`).
- **Arena-backed chunks** for cache locality.
- **Snapshot-capable** for undo/redo and future time-travel features
  without full buffer duplication.

Completely decoupled from rendering and input.

See `TEXT_ENGINE.md`.

## 6. Rendering Engine

- **GPU-accelerated via `wgpu`** (Vulkan / Metal / DX12 / GL fallback).
- **Batched draw calls**, **cached glyph atlases**, **visible-region-only
  rendering**.
- **Delta-only GPU updates**: only dirty lines re-shape; only dirty regions
  re-submit.
- **Frame-based**, target 60–120 fps.

See `RENDERING_PIPELINE.md`.

## 7. Input System

- Main-thread event-driven pipeline.
- **Direct-to-state** mapping: raw OS event → editor op, no intermediate
  command queue.
- Cursor updates and mutations applied in the same frame.
- MVP covers: typing, arrow keys, backspace, delete, newline, mouse click
  for cursor placement, scrolling.

See `INPUT_AND_IME.md`.

## 8. File System

- Async load; stream into rope.
- Memory-mapped reads for large files.
- Atomic writes via temp + fsync + rename.
- Decoupled from rendering.

See `FILE_IO.md`.

## 9. Concurrency

- Main thread: input, state mutation, frame orchestration.
- Render thread/task: owns wgpu resources.
- Worker pool (tokio multi-threaded runtime): file I/O and future
  background tasks.

See `CONCURRENCY.md`.

## 10. Observability

- `tracing` spans on every subsystem boundary.
- Input latency, frame time, memory, file load times recorded.
- Slow-operation warnings captured.
- Optional per-frame overlay (dev builds).

See `OBSERVABILITY.md`.

## 11. Extensibility (Future-Proofing, Not Built In MVP)

- Future plugins run sandboxed out-of-process (WASM).
- Resource limits enforced by runtime.
- Interfaces are defined early so adding them later does not reshape the
  core.

Explicitly **not** in MVP scope.

## 12. MVP Scope

**In:**

- Open a file from disk.
- View and edit text with a cursor.
- Smooth scrolling of large documents.
- Save the file back atomically.
- Cross-platform support (Windows primary; Linux and macOS in CI).

**Out:** Syntax highlighting, LSP, autocomplete, AI, plugins, themes,
tabs, project trees, terminal, settings UI.

## 13. Non-Functional Requirements

| Metric | Hard Target |
|---|---|
| Input-to-pixel latency | < 5 ms under normal load |
| Scrolling / editing frame rate | ≥ 60 fps (target 120) |
| Cold start | < 1 s |
| 100 MB file open | non-blocking |
| Memory | bounded; stable over hours of use |
| Crash / data-loss rate | zero in acceptance tests |

## 14. Success Criteria

MVP succeeds when, in side-by-side tests against VS Code and Cursor on the
same machine, the editor:

- Opens the same file faster.
- Types with less measured input-to-pixel latency.
- Scrolls a 100 MB file without dropped frames while the others stutter.
- Uses less RAM at idle.

Measurement methodology is in `PERFORMANCE_BUDGETS.md` and the M08
acceptance run records evidence in `STATUS.md`.

---

*Last updated: M00.*
