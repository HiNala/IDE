# M07 — Observability, Profiling, Dev Overlay

**Mission ID:** M07
**Prerequisites:** M06 complete. Editor opens, edits, and saves real files.
**Output:** The editor is now *provably* fast. Every subsystem emits structured tracing spans. A toggleable on-screen dev overlay shows p50/p95/p99 frame time, memory usage, scroll FPS, and last GPU submit time, updated in real time. Criterion benchmarks run in CI with regression alerts (>10% slower than the main-branch baseline fails the build). A `RUST_LOG=editor=trace` session produces a clean, readable trace that an engineer can use to diagnose any performance question.
**Estimated scope:** 1-2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/docs/PERFORMANCE_BUDGETS.md` — per-subsystem budgets.
- `/docs/TESTING_STRATEGY.md` — benchmarks and regression detection.
- `/reference/02_ARCHITECTURE_STRATEGY.md` §3, §10 — performance budgeting and CPU/GPU coordination.
- `https://docs.rs/tracing/` — span/event API.
- `https://docs.rs/tracing-subscriber/` — composing layers, `EnvFilter`, `fmt` format.
- `https://github.com/nagisa/tracing-tracy` — optional Tracy integration.

---

## The Situation In Plain English

We told ourselves from day one that performance is a first-class property. M07 is where we make sure that's actually true — and stays true as the code grows. Three things happen in this mission: we instrument, we visualize, and we guard.

Instrument: every subsystem boundary gets a `tracing` span. Input translation. Command application. Edit application. Rope mutation. Text layer prepare. GPU submit. File I/O. That means when someone runs the editor with `RUST_LOG=editor=trace` they see a clean, nested, timestamped transcript of exactly what each frame did and how long each phase took. No ad-hoc `println!`. No untyped log strings. Structured events with typed fields.

Visualize: an in-editor dev overlay, toggled with F11 or `--dev-hud`, shows live frame statistics pinned to the corner of the window. p50 / p95 / p99 frame times over the last second. Current fps. Last prepare time. Last submit time. Heap usage (via the memory-stats crate or simple platform APIs). This is the same kind of overlay you see in a game engine — it's immediate feedback during development, and it lets us catch a regression in the first minute of manual testing rather than on Monday when a customer complains.

Guard: Criterion benchmarks already exist from M02/M03/M04/M05/M06. We now wire them into CI such that a PR that makes `bench_insert_random_coherent` more than 10% slower than `main`'s stored baseline fails the build. We use `critcmp` or a small bash helper to parse Criterion's JSON output and do the comparison. This is the load-bearing part: without a regression guard, all the other observability gets slowly ignored.

We also add optional Tracy integration behind a feature flag. Tracy is a frame profiler built for games and real-time applications; it displays a flamegraph of tracing spans with microsecond resolution. It's overkill for day-to-day dev, but invaluable when chasing a subtle frame spike. We wire it up but keep it off by default.

---

## Scope

**In scope:**
- Tracing spans on every subsystem boundary.
- A `dev-hud` feature flag that, when enabled at runtime, renders an overlay showing live metrics.
- `MetricsCollector` in `editor-app` that aggregates frame timings, memory, and subsystem timings.
- Criterion-baseline comparison in CI with 10% regression threshold.
- Optional Tracy integration behind a `tracy` feature flag.
- A "performance test" smoke script (`scripts/perf-smoke.ps1` on Windows, `.sh` on Unix) that boots the editor with a 10 MB file, scripts some input, and fails if frame budget is exceeded.
- Better `tracing-subscriber` configuration: pretty format in debug builds, compact in release.

**Out of scope:**
- User-visible error toasts / dialogs (V2+).
- Telemetry / crash reporting (post-V2).
- Continuous performance monitoring service (post-V2).

---

## North Star

At the end of M07:

- A developer can open the editor, press F11, and immediately see a HUD showing "p50 8.1ms, p95 11.3ms, p99 14.0ms, mem 42MB, fps 120".
- `RUST_LOG=editor=debug` produces a readable timestamped stream of what each frame did.
- Tracy (if built with `--features tracy`) shows a flamegraph of spans with microsecond accuracy.
- CI fails any PR that slows `insert_random_coherent` by more than 10%.
- The project now has a continuous feedback loop that catches performance regressions automatically.

---

## TODO List

### 1. Adopt `tracing` everywhere it matters

- [ ] 1.1. Audit every crate for missing `tracing` usage.
- [ ] 1.2. Add `#[tracing::instrument(skip(...))]` to the following functions (skip large args to avoid log bloat):
  - `EditorState::apply` (skip `self`).
  - `TextLayer::prepare` (skip `self`, `buffer`, `gpu`).
  - `TextLayer::render` (skip `self`, `pass`).
  - `Renderer::render_frame` (skip everything except level).
  - `TextBuffer::insert` and `TextBuffer::delete` (skip `self`, `text`).
  - `UndoStack::push`, `undo`, `redo` (skip `self`, `edit`).
  - `save_file_sync` and `load_file_sync` (include `path`, skip `buffer`).
- [ ] 1.3. For hot paths (apply_edit, cursor motion), use `#[tracing::instrument(level = "trace")]` so they don't pollute `debug`-level output. Frame-level spans stay at `debug`.
- [ ] 1.4. Convert any remaining `println!`/`eprintln!` in non-test code to `tracing::*` calls.
- [ ] 1.5. Add structured fields: log cursor byte position, buffer version, frame number, phase durations as typed fields rather than format strings.
- [ ] 1.6. Commit: `refactor(tracing): instrument subsystem boundaries with structured spans`.

### 2. Build the `MetricsCollector`

- [ ] 2.1. `crates/editor-app/src/metrics.rs`:
  ```rust
  pub struct MetricsCollector {
      frame_times: VecDeque<Duration>,     // rolling 120-frame window
      prepare_times: VecDeque<Duration>,
      submit_times: VecDeque<Duration>,
      last_heap_sample: Option<HeapSample>,
      heap_sample_interval: Duration,
      last_heap_sample_at: Instant,
  }
  pub struct HeapSample { pub resident_bytes: u64, pub peak_resident_bytes: u64 }

  impl MetricsCollector {
      pub fn new() -> Self;
      pub fn record_frame(&mut self, prepare: Duration, submit: Duration, total: Duration);
      pub fn snapshot(&self) -> MetricsSnapshot;
  }
  ```
- [ ] 2.2. `MetricsSnapshot` is a plain data struct exposing p50/p95/p99 frame time, average fps, last prepare/submit, and the most recent `HeapSample`.
- [ ] 2.3. Heap sampling: use `memory-stats` crate for cross-platform RSS on Windows, macOS, Linux. Sample at most once per second (heap RSS is relatively expensive to query).
- [ ] 2.4. Unit test: feed synthetic timings, verify percentile computation.
- [ ] 2.5. Commit: `feat(app): introduce MetricsCollector with percentile and heap sampling`.

### 3. Wire `MetricsCollector` into the frame loop

- [ ] 3.1. Replace the ad-hoc `FrameTimer` percentiles from M05's frame loop with `MetricsCollector`. `FrameTimer` can stay as a low-level record-a-delta utility, or you can fold its functionality into `MetricsCollector`.
- [ ] 3.2. Emit a `debug!` line every 2 seconds with a full snapshot.
- [ ] 3.3. Commit: `refactor(app): route frame timings through MetricsCollector`.

### 4. Build the dev HUD overlay

- [ ] 4.1. New module `crates/editor-render/src/dev_hud.rs`. A simple `DevHud` struct that composes a short string from a `MetricsSnapshot` ("p50 8.1ms p95 11.3ms p99 14.0ms fps 120 mem 42MB") and uses `TextLayer` to render it at the top-right corner.
- [ ] 4.2. `DevHud::prepare` runs once per frame, right after `TextLayer::prepare`, and queues an additional `TextArea` with higher priority (small background quad drawn by `QuadLayer` so the HUD text is readable against any buffer content).
- [ ] 4.3. Visibility toggled by a boolean in `Renderer` (or passed per-frame in `FrameInput`). Default off. Toggle via F11 (wire the keybinding in `editor-input`; emit a new `EditorCommand::ToggleDevHud`).
- [ ] 4.4. If the `dev-hud` feature is disabled at compile time, `DevHud` is a no-op (enable via a Cargo feature on `editor-render` that defaults to on in debug builds and off in release builds).
- [ ] 4.5. Commit: `feat(render): implement DevHud overlay with metrics display`.

### 5. Set up Criterion baseline comparison in CI

- [ ] 5.1. In CI's `bench.yml` workflow, on pushes to `main`, run `cargo bench` and commit the Criterion results to a branch `perf-baselines` (or upload them as a GitHub Actions artifact; artifacts are simpler and don't require repo write access). Use `cargo bench -- --save-baseline main`.
- [ ] 5.2. On PR builds, download the latest `main` baseline artifact, run `cargo bench -- --baseline main --save-baseline pr`, and compare via `critcmp`. Add `critcmp` to the workflow's install step.
- [ ] 5.3. Parse `critcmp`'s output. If any benchmark is > 10% slower, fail the workflow. If any benchmark is > 25% slower, fail loudly with a clear error message pointing at the specific regression.
- [ ] 5.4. Allow override via a `perf-allow-regression` label on the PR — sometimes a known regression is intentional (e.g., in exchange for correctness). Document this in `/CONTRIBUTING.md`.
- [ ] 5.5. Commit: `ci(bench): add Criterion baseline comparison with 10% regression gate`.

### 6. Wire optional Tracy support

- [ ] 6.1. Add a `tracy` feature to `editor-app` (and to `editor-render` if Tracy needs GPU frame markers, which it does — but for MVP we can skip GPU-side profiling and only profile CPU).
- [ ] 6.2. When the feature is on, initialize `tracing-tracy` as a subscriber layer alongside the fmt layer.
- [ ] 6.3. Document how to use Tracy in `/docs/PERFORMANCE_BUDGETS.md` (install the Tracy viewer app, build the editor with `cargo run --release --features tracy`, connect).
- [ ] 6.4. Commit: `feat(app): optional Tracy profiler via tracing-tracy`.

### 7. Build the performance smoke test

- [ ] 7.1. `scripts/perf-smoke.ps1` (Windows) and `scripts/perf-smoke.sh` (Unix) that: generate a 10 MB Lorem-Ipsum file, launch `editor-app` with a special `--perf-smoke` flag that disables interactive input, scripts a sequence (load file → scroll through whole buffer → jump to end → type 100 chars → undo all → save to a temp file), captures metrics on exit, and fails if p99 > 16 ms or if any single frame > 50 ms.
- [ ] 7.2. `--perf-smoke` mode: a hardcoded scripted sequence of `EditorCommand`s applied programmatically, with metrics captured and logged to stdout as JSON.
- [ ] 7.3. Run this in CI on Windows and Ubuntu (skip macOS for now — Ubuntu has llvmpipe which is slow and unrepresentative, so note in CI that the Ubuntu check only verifies "doesn't crash", not performance; the Windows check is the real one).
- [ ] 7.4. Commit: `test(perf): add --perf-smoke scripted regression test`.

### 8. Log format and level conventions

- [ ] 8.1. In `editor-app/src/main.rs`, configure `tracing-subscriber` thoughtfully:
  - Debug build: `fmt::layer().pretty()` with human-readable format, target filter defaulting to `info` but `EnvFilter::from_default_env()` override.
  - Release build: `fmt::layer().compact()` with default `warn` level.
  - JSON format available behind `--log-json` flag (useful for downstream log shipping even for a local app).
- [ ] 8.2. Document convention in `/docs/RUST_CONVENTIONS.md`:
  - `trace!`: inner-loop diagnostics, off in all default configs.
  - `debug!`: frame-level timings, subsystem transitions.
  - `info!`: startup, shutdown, file load/save completions.
  - `warn!`: recoverable problems (atlas full retry, external file change detected, frame exceeded budget).
  - `error!`: unrecoverable problems that don't crash the process (save failed, load failed, GPU device lost).
- [ ] 8.3. Commit: `refactor(logging): finalize subscriber configuration and level conventions`.

### 9. Document how to diagnose performance issues

- [ ] 9.1. New `/docs/DIAGNOSING_PERFORMANCE.md`. Cookbook:
  - "The editor feels slow on my machine": check F11 overlay; if p99 is fine, it's input latency, enable `latency-trace`; if not, look at prepare vs submit split.
  - "One specific file is slow": load it with `RUST_LOG=editor_render=debug` and look for atlas-full warnings; consider font fallback chains.
  - "Large file opening is slow": check whether mmap succeeded (the log says); check disk read throughput externally.
  - "Scrolling stutters sporadically": enable Tracy, look for gaps in the frame timeline.
- [ ] 9.2. Commit: `docs: add DIAGNOSING_PERFORMANCE.md cookbook`.

### 10. Benchmarks for new code paths

- [ ] 10.1. `MetricsCollector::record_frame` is called every frame — must be cheap. Benchmark: sub-microsecond.
- [ ] 10.2. `MetricsCollector::snapshot` — called 30x/sec for the HUD — must also be cheap. Benchmark.
- [ ] 10.3. `DevHud::prepare` — under 200 μs per frame.
- [ ] 10.4. Save baseline as `m07-mvp`.
- [ ] 10.5. Commit: `bench(app, render): metrics and HUD overhead benchmarks`.

### 11. Quality gates

- [ ] 11.1. `cargo fmt --all --check`.
- [ ] 11.2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] 11.3. `cargo test --workspace`.
- [ ] 11.4. `cargo bench` across the workspace.
- [ ] 11.5. Run `scripts/perf-smoke.ps1` on Windows; confirm green.
- [ ] 11.6. Manual: enable HUD, scroll through a 10 MB file, verify numbers look sane.

### 12. Documentation & handoff

- [ ] 12.1. Update `/docs/PERFORMANCE_BUDGETS.md` with reference numbers from your current machine (CPU model, GPU, OS, baseline frame times).
- [ ] 12.2. Update `/docs/STATUS.md`: M07 complete, M08 next.
- [ ] 12.3. Update `/CHANGELOG.md`.
- [ ] 12.4. Tag: `git tag -a m07-complete -m "M07 complete: observability and regression guard"`; push.

---

## Validation / Acceptance Criteria

M07 is complete when:

1. Quality gates pass.
2. CI green on all three OSes.
3. `RUST_LOG=editor=debug cargo run` produces a clean, readable stream with structured fields on startup, frame timings every 2 s, and file I/O completion events.
4. F11 toggles the dev HUD, which updates in real time.
5. CI fails a PR that regresses `insert_random_coherent` by >10%.
6. `--perf-smoke` runs and reports pass/fail with captured JSON metrics.
7. Optional Tracy build works (`cargo run --release --features tracy`) and connects to the Tracy viewer.
8. `docs/DIAGNOSING_PERFORMANCE.md` exists.
9. `m07-complete` tag pushed.

## Testing Requirements

- Unit tests for `MetricsCollector` percentiles.
- `--perf-smoke` smoke test in CI (Windows).
- Manual HUD toggling.
- Regression guard verified by a deliberate slow-down PR (optional but recommended — then revert it).

## Git Commit Strategy

10-14 commits. Push after items 2, 4, 5, 7, 9, 12.

## Handoff to M08

M08 assumes:

- Performance is observable and continuously guarded.
- The editor is feature-complete for MVP (open, edit, save, fast).
- All subsystems are instrumented enough that M08's acceptance testing can be data-driven.

---

## Standing Orders Reminder

- Tracing is free if you let it be free. Use `tracing::instrument` — it compiles to almost nothing when the level is disabled. Use structured fields (`field = %value`) rather than formatted strings when you can.
- The regression threshold (10%) is calibrated to be tight enough to catch real problems and loose enough to accommodate noise. If you see CI fail on unrelated PRs due to noise, the fix is usually to increase Criterion's sample count, not to raise the threshold.
- The HUD is developer-facing. It should not be in release builds by default. Keep it behind a feature flag.

Go.
