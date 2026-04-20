# Concurrency Model

The editor uses ownership-based, message-driven, lock-light concurrency.
Rust's ownership rules are treated as a **structural guarantee**, not a
decoration.

## 1. Threads & Runtimes

```
┌───────────────────────────────┐
│         MAIN THREAD           │   winit event loop + editor-input +
│                               │   editor-core mutations + frame
│    (real-time path)           │   orchestration
└────────────┬──────────────────┘
             │ arc-swap / channels
             ▼
┌───────────────────────────────┐
│    RENDER TASK (or thread)    │   owns wgpu Device/Queue/Surface;
│                               │   consumes RenderSnapshot per frame
│    (real-time path)           │
└───────────────────────────────┘
                  ▲
                  │ metrics
                  │
┌───────────────────────────────┐
│       TOKIO WORKER POOL       │   file I/O, future indexing,
│  multi-thread, spawn_blocking │   future LSP client
│  allowed                      │
│    (background path)          │
└───────────────────────────────┘
```

**Rules**

- Main thread owns the `Document` and the event loop.
- Render task is the only owner of wgpu resources.
- Worker pool runs background `async` tasks; never holds a reference to
  the `Document` directly. It produces `Load` results and receives
  "save this buffer" requests.

## 2. Render Task Placement

Two acceptable configurations:

- **Async render** — render runs as a `tokio::task::spawn_local` on a
  dedicated single-threaded runtime, blocked on a bounded channel of
  render snapshots.
- **Dedicated render thread** — `std::thread::Builder::new().spawn(...)`,
  consuming from a `crossbeam_channel`.

We start with **dedicated render thread** in M03; it's simpler and avoids
coupling the render code to tokio. Revisit during M07 if profiling says
otherwise.

## 3. Snapshot Publication

The render consumer does not hold a `Mutex<Document>`. It reads an
`Arc<RenderSnapshot>` published via [`arc_swap::ArcSwap`]:

```rust
struct SharedState {
    snapshot: arc_swap::ArcSwap<RenderSnapshot>,
    metrics:  tracing_metrics::Counters,
}
```

Every frame on the main thread:

1. Mutate `Document`.
2. Build a new `RenderSnapshot` (cheap — rope is structurally shared).
3. `shared.snapshot.store(Arc::new(new_snapshot))`.

Every frame on the render thread:

1. `shared.snapshot.load_full()`.
2. Compute dirty regions vs. the last snapshot it rendered.
3. Shape and draw only those regions.

`ArcSwap` is lock-free on the read side and near-lock-free on the write
side. This is the right tool for read-mostly, occasional-write published
state.

## 4. Channels

- **Worker → main:** `crossbeam_channel::bounded(32)`. Used for
  "file loaded", "file saved", "error" events. Drained on each main
  iteration.
- **Main → worker:** `tokio::sync::mpsc::channel(32)` for submitting
  background tasks with back-pressure.
- **Main → render:** the `ArcSwap` above; no channel necessary.
- **Render → main (metrics):** `crossbeam_channel::bounded(256)`; drops
  oldest on overflow because metrics are cache, not truth.

Bounded everywhere. No unbounded channels in real-time paths.

## 5. Locking

- `parking_lot::Mutex` / `RwLock` are permitted only outside the real-time
  path (e.g. guarded configuration reloaded from disk).
- **No lock on the hot path.** This is an invariant; violating it fails
  review.
- `std::sync::atomic::AtomicU64` for simple shared counters (frame counter,
  document version).

## 6. Cancellation

Background tasks are cancellable via dropping their `JoinHandle` or
`tokio::task::AbortHandle`. Two categories:

- **Idempotent:** file loads. Cancelling drops the partial result.
- **Side-effectful:** saves. Cancelling is **not** supported mid-write to
  preserve atomic-save guarantees. The save task completes or fails as a
  unit.

## 7. Panics & Fault Isolation

- A panic on the main thread crashes the process (there is no meaningful
  recovery for a corrupt document).
- A panic on the render thread triggers a controlled restart: catch via
  `std::panic::catch_unwind` around the render loop, recreate all GPU
  resources, resume.
- A panic on a worker task is logged and isolated; the main thread
  continues.

## 8. Determinism

Our main concurrency pattern (single-writer `Document`, immutable
snapshots, bounded FIFO channels) produces deterministic output for a
given input stream. Background tasks are not deterministic and must not
influence document state except through well-defined events
(`FileLoaded { contents }`).

## 9. Back-Pressure

If the worker pool falls behind, the `tokio::sync::mpsc` channel's bound
applies back-pressure to the main thread's `send`. The main thread
handles the result by:

- Failing the operation (e.g. "queue full, try again").
- Logging via `tracing::warn!`.

The UI never stalls; it only refuses new background work.

## 10. Test Strategy

- **Loom** (optional, in M07 or later) for deterministic concurrency
  model checking on the snapshot + channel machinery.
- **Integration tests** that spawn a minimal headless app, drive
  actions, and assert per-frame invariants.
- **Stress test** in M08: hammer the worker pool with concurrent loads
  and saves while typing.

## 11. Concurrency Checklist For New Code

When introducing a new piece of work:

1. Does it run in the frame's 8.33 ms budget? → main thread or render
   thread, synchronous.
2. Does it touch the filesystem, network, or heavy computation? →
   worker pool, `tokio::spawn`.
3. Does it need to return data to the UI? → bounded channel, polled each
   frame.
4. Does it need to share state? → `ArcSwap` snapshot, not `Mutex`.
5. Does it need locking? → justify in the commit message; PR requires a
   note on why.

---

*Last updated: M00.*
