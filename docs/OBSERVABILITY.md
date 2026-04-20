[← docs/](./) · [README](../README.md)

# Observability

The performance contract is measurable or it isn't real. This file
documents what we measure, how, and where it shows up.

## 1. Goals

- Make per-frame behavior visible during development.
- Make regressions detectable in CI.
- Make post-mortem diagnosis practical without a debugger.
- **Cost zero in release builds without `dev-overlay` enabled.**

## 2. Tracing Infrastructure

We use the [`tracing`](https://docs.rs/tracing) ecosystem.

- **`tracing::span!`** at every subsystem boundary and for every frame
  phase.
- **`tracing::event!`** for significant state transitions (file
  loaded, save complete, GPU device lost).
- **`tracing_subscriber::fmt`** for default text output.
- **`tracing_subscriber::EnvFilter`** honoring `RUST_LOG`.
- **`tracing-chrome`** exporter gated by a `--trace-chrome path.json`
  CLI flag.

`editor-app` initializes the subscriber at startup. Library crates never
initialize subscribers.

## 3. Span Naming Conventions

- Frame phases: `frame.input`, `frame.mutate`, `frame.layout`,
  `frame.render`, `frame.submit`, `frame.present`.
- Subsystem operations: `editor_core.insert`, `editor_core.delete`,
  `editor_core.undo`, `editor_io.load`, `editor_io.save`,
  `editor_render.reshape_line`.
- Background tasks: `worker.file_load`, `worker.file_save`.

Each frame-level span carries a `frame_id: u64` field.

## 4. Metrics

Recorded per frame and published through `arc-swap` to the dev overlay:

```rust
pub struct FrameMetrics {
    pub frame_id: u64,
    pub phase_ns: [u64; 6],            // one per frame phase
    pub gpu_queue_depth: u32,
    pub rope_bytes: u64,
    pub rope_chunks: u32,
    pub visible_lines: u32,
    pub dirty_lines_shaped: u32,
    pub cursor_byte: u64,
    pub rss_bytes: u64,                // sampled every 60 frames
    pub last_input_arrival_ns: u64,    // for latency overlay
}
```

Ring-buffered (64 frames) for sparkline-style overlay rendering.

## 5. Dev Overlay

Toggled by `F1`. Renders a compact panel in the top-right corner:

```
frame_id      12,345
phase_us      in:  43  mut:  87  lay: 210  enc: 180  sub: 320  pre:  50
gpu_queue     1
rope          rope=128 MB chunks=67 lines=1,045,210
visible       212 lines · dirty 3
rss           142 MB
input→submit  3.2 ms (p50) · 4.7 ms (p99)
backend       DX12 (NVIDIA RTX)
fps           238 (avg 120)
```

Implementation: the overlay is a small separate wgpu render pass with
its own (tiny) atlas. It never blocks or interleaves with the main text
pass in a way that could affect measured latency.

**Crucially:** with the overlay disabled (default), metric collection is
skipped entirely. The `dev-overlay` Cargo feature gates the clock reads
and the metric struct updates.

## 6. Feature Flags

| Flag | Default | Effect |
|---|---|---|
| `dev-overlay` | off | Compiles overlay code, enables per-frame metric collection. |
| `trace-chrome` | off | Enables the `--trace-chrome` flag and dependency on `tracing-chrome`. |
| `loom` | off | Replaces `std::sync` primitives with `loom` for model-checked tests. |

Feature flags ship with clear documentation in each crate's `README.md`.

## 7. Benchmarks

Criterion benches in every crate's `benches/` directory. Baseline
comparison in CI:

```bash
# First run on main
cargo bench -- --save-baseline main

# On PRs
cargo bench -- --baseline main
```

PRs fail if any benchmark regresses by > 5 %. The tolerance can be
overridden per-benchmark in Criterion's config. See
`TESTING_STRATEGY.md` §5 for the exact bench list.

## 8. Logs At Runtime

Logs go to stderr via `tracing_subscriber::fmt::layer()`. Format:

```
2026-04-20T07:51:02.123Z  INFO editor_render: backend=Dx12 adapter="NVIDIA RTX 4070"
2026-04-20T07:51:02.456Z  WARN editor_render: phase.submit=18ms (budget 2ms)
2026-04-20T07:51:03.000Z  INFO editor_io::save: atomic write complete path=/tmp/foo.rs bytes=12345
```

`RUST_LOG` controls verbosity. Sensible default: `warn` for libraries,
`info` for `editor_app`.

## 9. Crash / Panic Reporting

- Panic hook prints the panic message plus a recent `tracing` log
  snapshot to stderr.
- On Windows, enable `SetUnhandledExceptionFilter` via `winit`'s default
  to at least flush before termination.
- No crash telemetry in MVP. Adding it is explicitly a future mission.

## 10. Memory Sampling

- Sample RSS every 60 frames (≈ once per second at 60 fps).
- Windows: `GetProcessMemoryInfo`.
- Linux: `/proc/self/status`, parse `VmRSS`.
- macOS: `task_info` with `MACH_TASK_BASIC_INFO`.
- Wrap in a tiny `editor_observability::rss()` helper.

## 11. Profiling Workflows

- **`cargo flamegraph`** (with `perf` or `dtrace`) for CPU profiling on
  Linux/macOS.
- **`samply`** (recommended) for cross-platform sampling profiler that
  exports to Firefox Profiler format.
- **`tracy`** (optional) for frame-granular profiling; hooked via
  `tracing-tracy` under a feature flag.
- **Chrome Trace** via `tracing-chrome` for a zero-setup view of spans.

## 12. M08 Acceptance Telemetry

The M08 acceptance run dumps a single JSON summary to
`docs/benchmarks/m08.json`:

```json
{
  "os": "windows",
  "adapter": "DX12 NVIDIA RTX 4070",
  "latency_p50_ms": 2.9,
  "latency_p99_ms": 4.6,
  "scroll_fps_min": 91,
  "cold_start_ms": 620,
  "file_100mb_open_ms": 380,
  "soak_rss_start_mb": 108,
  "soak_rss_end_mb": 112
}
```

That file is committed as the mission's acceptance artifact.

---

*Last updated: M00.*
