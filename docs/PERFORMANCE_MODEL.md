# Performance Model

This is the **binding performance contract** for the editor. Every commit is
evaluated against it.

If a change regresses any target, the change is wrong. Use Criterion to
prove performance; do not ship on intuition.

## 1. Hard Targets

| # | Metric | Target | Measured In |
|---|---|---|---|
| P1 | Input-to-pixel latency | p50 < 3 ms, p99 < 5 ms under normal load | M05 |
| P2 | Frame rate during scroll | ≥ 60 fps (target 120 fps) | M04, M05, M08 |
| P3 | Frame rate during edit | ≥ 60 fps | M05, M08 |
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

See `docs/TESTING.md` for concrete harness locations.

## 5. Budget Enforcement

Each subsystem owns a budget. When a subsystem exceeds its budget, the
loop must:

1. **Defer** — split work across frames.
2. **Shed** — skip the non-essential portion (e.g. extra metric samples).
3. **Coalesce** — merge work (e.g. multiple edits in one layout pass).

A soft warning is emitted via `tracing::warn!` when any phase overruns
its budget in a frame. A hard panic is never acceptable in the hot path.

## 6. Memory Budgets

| Allocator / Buffer | Budget | Notes |
|---|---|---|
| Rope chunks | ~1.2× document size | ropey default settings are close to this. |
| Undo/Redo history | cap at 64 MiB | Coalesce + compact on milestones. |
| Glyph atlas | 64 MiB GPU-resident cap | Evict least-recently-used glyphs. |
| Layout cache | 16 MiB | Per-line shaping cache; LRU-evicted. |
| Frame arenas | 1 MiB × 2 (double-buffered) | Reset each frame. |
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
