# Architecture Strategy & Performance Model

This file bundles the **architecture overview** and **performance budgets** from the project \docs/\ tree for PRD traceability. Authoritative editing still happens in:

- \docs/ARCHITECTURE.md\
- \docs/PERFORMANCE_BUDGETS.md\

---

# Architecture

This is the **canonical architecture reference** for the IDE project. It is
kept short and decisive. Long-form design discussion lives in the sibling
documents in this `docs/` tree and is linked below.

> **Rule of thumb:** if this document disagrees with another `docs/` file,
> this document wins and the sibling doc is updated. If a mission changes the
> shape of the system, update this file in the same commit.

## North Star

Deliver a native code editor that, on identical hardware, responds to input,
scrolls, and opens multi-megabyte files visibly and measurably faster than
VS Code or Cursor â€” and keeps that advantage as features accrete.

## Core Model: A Real-Time Engine for Text

The editor is designed as a **real-time interactive engine** whose frame loop
is analogous to a game engine, not a productivity app. Every frame consists of
three ordered phases with strict per-phase budgets:

1. **Input** â€” Consume OS events, translate to editor operations, apply to
   document state. Bounded by milliseconds.
2. **State Mutation** â€” Deterministic, incremental mutation of the rope-backed
   document and derived viewport state. Proportional to edit size, not
   document size.
3. **Render** â€” Compute dirty screen regions only; submit delta draw calls to
   the GPU. Bounded by viewport size, not file size.

Background work (file I/O, future indexing, future LSP) runs on a separate
worker pool and is **never allowed to appear in the real-time frame path**.

See `PERFORMANCE_BUDGETS.md` for the full budget table.

## Layered Subsystem Map

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚                          editor-app (binary)                          â”‚
â”‚   Wires subsystems, owns the winit event loop, drives the frame loop. â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
                â”‚                 â”‚                â”‚
        â”Œâ”€â”€â”€â”€â”€â”€â”€â–¼â”€â”€â”€â”€â”€â”€â”€â”  â”Œâ”€â”€â”€â”€â”€â”€â–¼â”€â”€â”€â”€â”€â”€â”  â”Œâ”€â”€â”€â”€â”€â”€â–¼â”€â”€â”€â”€â”€â”€â”
        â”‚ editor-input  â”‚  â”‚ editor-core â”‚  â”‚editor-renderâ”‚
        â”‚ OS events â†’   â”‚  â”‚ Rope text,  â”‚  â”‚ wgpu, glyph â”‚
        â”‚ editor ops    â”‚  â”‚ cursor, UR  â”‚  â”‚ atlas, pass â”‚
        â””â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”˜  â””â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”˜  â””â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”˜
                â”‚                 â”‚                â”‚
                â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”˜
                          â”‚                â”‚
                     â”Œâ”€â”€â”€â”€â–¼â”€â”€â”€â”€â”     â”Œâ”€â”€â”€â”€â”€â–¼â”€â”€â”€â”€â”€â”
                     â”‚editor-ioâ”‚     â”‚observabilityâ”‚
                     â”‚async FS â”‚     â”‚ metrics +  â”‚
                     â”‚ atomic  â”‚     â”‚ tracing    â”‚
                     â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜     â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

**Crate boundaries = subsystem boundaries.** No crate reaches across the
diagram sideways. The only crate that depends on everything is `editor-app`.

| Crate | Role | Real-time? |
|---|---|---|
| `editor-core` | Rope buffer, cursor, selection, undo/redo snapshots, document model | Yes |
| `editor-render` | wgpu device/queue, render pipelines, glyph atlas, layout â†’ draw | Yes |
| `editor-input` | OS event â†’ editor command mapping, IME state, key bindings | Yes |
| `editor-io` | Async file load, memory-mapped reads, atomic save, line-ending normalization | No (background) |
| `editor-app` | Binary: window, frame loop, subsystem wiring, dev overlay | Mixed |

Observability (`tracing` spans, frame metrics) is cross-cutting and exposed as
a thin module inside `editor-app` plus lightweight hooks in each subsystem.

## State Model

Two coexisting styles, chosen per-subsystem for the right reason:

- **Rope buffer (incremental mutation).** The document itself. Edits are
  O(log n) local mutations; there is never a full copy on keystroke.
- **Per-frame immutable views (snapshot-style).** Selection, cursor set,
  viewport metrics, dirty regions. Each frame computes an immutable view from
  the mutated document; the renderer consumes that view. This eliminates
  cross-thread aliasing and race conditions in the render path.

Undo/redo uses rope-native reversible operations (insert/delete records), not
full snapshots, so memory stays bounded during long editing sessions.

## Concurrency Model

Ownership-based, message-driven, lock-light.

- **Main thread:** owns `winit` event loop, input translation, state mutation,
  and frame orchestration.
- **Render thread or async render task:** owns the wgpu `Device`, `Queue`, and
  all GPU resources. Receives a per-frame immutable render packet over a
  bounded channel.
- **Worker pool:** short-lived cancellable tasks on `tokio`'s multi-threaded
  runtime (file load, large-save, future indexing). Never touches the real-time
  path directly.

No locks on the hot path. Channels are bounded; back-pressure is explicit.

See `CONCURRENCY.md`.

## Rendering Strategy

- Single wgpu `Surface` per window; `Present Mode` chosen at runtime
  (`Mailbox` where supported, `Fifo` as safe fallback).
- GPU-resident glyph atlas; CPU-side shaping via `cosmic-text`/`glyphon`.
- Dirty-rect tracking at the line level; only invalidated lines re-encode.
- One render pass per frame in MVP; subpasses are added only when profiling
  shows a win.

See `RENDERING_PIPELINE.md`.

## File I/O Strategy

- Small/medium files: async streaming load into the rope.
- Large files (> configurable threshold; default 16 MiB): memory-mapped read
  with on-demand chunk decoding.
- Saves: write to temp file in the same directory, `fsync`, atomic rename.
- Line endings: normalize to `\n` internally; preserve original on save unless
  the user explicitly opts into conversion.

See `FILE_IO.md`.

## Cross-Platform Strategy

Primary development on Windows. CI runs on Windows, Linux, and macOS for every
PR. All platform-divergent code is gated with `#[cfg(target_os = "...")]` or
encapsulated behind a single crate-internal trait with per-OS impls. Paths are
always `std::path::Path`/`PathBuf`; file content uses `\n` internally.

See `CROSS_PLATFORM.md`.

## Observability

- `tracing` for structured logging with per-span durations.
- A dev-only overlay (togglable with `F1`) shows per-phase frame timings,
  GPU queue depth, rope statistics, and peak memory.
- Criterion benchmarks gate PRs on the hot paths (rope edits, layout,
  atlas lookups).

See `OBSERVABILITY.md`.

## What This Architecture Explicitly Rejects

- Any Electron/Chromium/webview embedding.
- Any dynamic scripting runtime in the hot path (plugins, if they come, run
  sandboxed out-of-process).
- Any unbounded per-frame work. If it cannot fit a frame budget, it is
  background work.
- Any global mutable state reached through statics or `OnceLock`-style shared
  handles in the hot path.
- Any Cargo feature flag that changes observable behavior without being
  documented here and in the crate where it lives.

## Evolution Rules

- New subsystems live in new crates.
- A new crate must declare its real-time vs. background classification in
  its own `README.md` plus an entry in this file.
- Adding a dependency requires a one-sentence justification in the commit
  message and, if it is non-trivial (>50 KLOC compiled), a short note in
  `TECH_STACK.md`.
- Performance regressions detected by Criterion block the commit. No
  exceptions without an explicit `FOLLOWUPS.md` entry and the sign-off of the
  next mission planner.

## Related Documents

- `TECH_STACK.md` â€” dependency-level decisions and rationale.
- `PERFORMANCE_BUDGETS.md` â€” per-frame budgets and measurement methodology.
- `TEXT_ENGINE.md` â€” rope internals and cursor math.
- `RENDERING_PIPELINE.md` â€” wgpu pipeline, glyph atlas, layout.
- `INPUT_AND_IME.md` â€” OS event â†’ edit op translation and IME flow.
- `CONCURRENCY.md` â€” ownership and message-passing model.
- `FILE_IO.md` â€” load/save/mmap/atomic-write strategy.
- `CROSS_PLATFORM.md` â€” Windows/Linux/macOS divergence.
- `OBSERVABILITY.md` â€” tracing, metrics, dev overlay.
- `TESTING_STRATEGY.md` â€” unit, integration, property, benchmark strategy.
- `RUST_CONVENTIONS.md` â€” coding style, error handling, logging.
- `RISKS.md` â€” known gaps and mitigations.
- `MISSIONS.md` â€” mission index and execution order.
- `STATUS.md` â€” current mission state.

---


# Performance Model

This is the **binding performance contract** for the editor. Every commit is
evaluated against it.

If a change regresses any target, the change is wrong. Use Criterion to
prove performance; do not ship on intuition.

## 1. Hard Targets

| # | Metric | Target | Measured In |
|---|---|---|---|
| P1 | Input-to-pixel latency | p50 < 3 ms, p99 < 5 ms under normal load | M05 |
| P2 | Frame rate during scroll | â‰¥ 60 fps (target 120 fps) | M04, M05, M08 |
| P3 | Frame rate during edit | â‰¥ 60 fps | M05, M08 |
| P4 | Cold start | < 1 s on SSD, modern hardware | M08 |
| P5 | 100 MB file open | non-blocking UI, visible < 500 ms | M06, M08 |
| P6 | 4-hour soak RSS growth | < 10 % | M08 |
| P7 | Crash rate | zero panics in acceptance runs | M08 |

"Normal load" means: one document open, window focused, no background
indexing (none exists in MVP).

## 2. Frame Budget

At 60 fps we have 16.66 ms per frame; at 120 fps, 8.33 ms. We budget against
the tighter target so there's headroom on slower hardware.

### 8.33 ms Frame Budget (120 fps target)

| Phase | Budget | Notes |
|---|---|---|
| Input collection | 0.2 ms | Drain `winit` events, translate to ops. |
| State mutation | 0.5 ms | Rope edits, cursor update. |
| Layout + shaping (incremental) | 1.5 ms | Only dirty lines. |
| Render encoding | 1.0 ms | Build the command buffer. |
| GPU submit + present | 2.0 ms | Submit and swap. |
| Observability | 0.1 ms | Per-frame metric record. |
| **Total budget used** | **5.3 ms** | Leaves ~3 ms headroom. |

If a phase exceeds its budget, it must be split across frames or deferred.
Unbounded work in the frame path is a bug.

### 16.66 ms Frame Budget (60 fps minimum)

Double the budgets above; this gives us the floor for slower hardware.

## 3. Input-to-Pixel Latency Model

```
t0 : OS kernel delivers input event to process
t1 : winit event loop emits WindowEvent
t2 : editor-input maps to EditOp
t3 : editor-core applies mutation, marks dirty lines
t4 : layout/shaping of dirty lines
t5 : render encoder built
t6 : wgpu submits to GPU queue
t7 : GPU finishes + present
t8 : display pixel change visible to photons

latency = t8 - t0
```

We cannot measure t0 and t8 directly without external hardware. Our
software proxy is `t1..t6` plus a modeled display latency derived from
present mode:

- `Mailbox`: add ~0.5 frames average display latency.
- `Fifo`: add ~1.0 frames average display latency.
- `Immediate` (if enabled on Windows via DX12): ~0 but may tear.

A stretch goal (post-M07) is optional high-speed camera validation on one
reference machine. For MVP the software proxy plus the present-mode model
is sufficient.

## 4. Measurement Methodology

- **Criterion benchmarks** in each crate, gated by CI, run on every PR on
  at least `ubuntu-latest`.
- **`criterion`'s regression detection** treats a 5 % slowdown on a hot
  path as a failure.
- **Frame time traces** collected by the dev overlay (M07) export to
  Chrome Trace via `tracing-chrome`.
- **Wall-clock startup** measured by a small Rust harness that spawns the
  binary and times window creation via an external signal (printing to
  stdout when the first frame is submitted).
- **Soak tests** scripted via `xdotool`/`AutoHotkey`/`osascript` driving
  the real binary. Memory sampled by reading RSS through platform APIs.

See `TESTING_STRATEGY.md` for concrete harness locations.

## 5. Budget Enforcement

Each subsystem owns a budget. When a subsystem exceeds its budget, the
loop must:

1. **Defer** â€” split work across frames.
2. **Shed** â€” skip the non-essential portion (e.g. extra metric samples).
3. **Coalesce** â€” merge work (e.g. multiple edits in one layout pass).

A soft warning is emitted via `tracing::warn!` when any phase overruns
its budget in a frame. A hard panic is never acceptable in the hot path.

## 6. Memory Budgets

| Allocator / Buffer | Budget | Notes |
|---|---|---|
| Rope chunks | ~1.2Ã— document size | ropey default settings are close to this. |
| Undo/Redo history | cap at 64 MiB | Coalesce + compact on milestones. |
| Glyph atlas | 64 MiB GPU-resident cap | Evict least-recently-used glyphs. |
| Layout cache | 16 MiB | Per-line shaping cache; LRU-evicted. |
| Frame arenas | 1 MiB Ã— 2 (double-buffered) | Reset each frame. |
| Tokio runtime | 64 MiB stack pool | Typical for multi-thread runtime. |

Totals vary by document; the invariant is **bounded over time**.

## 7. Hardware Adaptation

At boot, `editor-app` reads:

- CPU core count (`std::thread::available_parallelism`).
- Memory (via platform APIs; optional for MVP).
- GPU info (from `wgpu::Adapter::get_info`).

From those it configures:

- Tokio worker thread count (cap at physical cores).
- Atlas size (cap at adapter's max texture dimension / 4).
- Present mode (prefer `Mailbox`, fall back to `Fifo`).
- Text rendering pathway (always GPU in MVP; future CPU fallback if an
  adapter fails).

## 8. Regression Detection

- CI runs `cargo bench -- --save-baseline ci` on `main` merges.
- PRs run benches and `cargo bench -- --baseline ci`; a > 5 % regression
  on any hot path fails the PR.
- The hot-path list is maintained in each crate's `benches/` directory
  with a single `README.md` describing what each bench measures.

## 9. Anti-Patterns (Avoid At All Costs)

- **Rebuilding the full layout per frame.** Use dirty lines.
- **Allocating per-frame from the global allocator in the hot path.**
  Use frame arenas.
- **Locking on the hot path.** Use lock-free channels (`crossbeam`) and
  `arc-swap` snapshots.
- **Submitting to the GPU mid-frame on the main thread.** Use the
  render-thread boundary.
- **Doing file I/O from the main thread.** Use the worker pool.
- **Calling `.await` on anything inside the 8.33 ms budget.** All awaits
  belong in background tasks.

---

*Last updated: M00.*
