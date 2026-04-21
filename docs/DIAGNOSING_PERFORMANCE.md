[← docs/](./) · [README](../README.md)

# Diagnosing performance

Cookbook for when the editor feels slow or stutters. Expanded in **M07** (observability) and **M08** (acceptance).

## Quick checks

1. **Dev HUD / metrics** — Press **Ctrl+F11** (or start with `--dev-hud`). The metrics
   line (p50 / p95 / p99, fps, prepare vs GPU, RSS) renders in the **viewport**;
   the **title bar** switches to buffer stats while the HUD is on. `RUST_LOG=editor_app=debug`
   logs a structured metrics snapshot (`editor_app::metrics::MetricsSnapshot`) about every two seconds while frames run.
2. **Tracing** — `RUST_LOG=editor_app=debug,editor_render=debug` for frame and subsystem spans.
3. **Tracy** — Optional `--features tracy` build; connect the Tracy viewer for flamegraphs.

## Symptom → action

| Symptom | What to check |
|---------|---------------|
| Feels laggy but HUD p99 is fine | Input path vs render path; enable trace on `editor-input`. |
| One large file is slow | Load path (mmap vs stream), atlas/glyph warnings in `editor_render` (M04+). |
| Open is slow | Disk throughput; whether mmap succeeded (logs in `editor_io`, M06+). |
| Scrolling stutters | Tracy timeline; GPU submit vs prepare split. |

## GPU device lost

If `wgpu` reports device loss, the app should attempt one recovery (M08 polish). If recovery fails, exit code **2** is reserved (see `DEVELOPMENT.md`). Until implemented, check logs for `ERROR` from `editor_render`.

## Resize (M12)

Typical costs during window resize:

1. **Swapchain reconfiguration** — `GpuContext::resize` updates `SurfaceConfiguration` and calls `surface.configure`. The swapchain is recreated by the driver; keep document shaping out of this path where possible.
2. **Row scratch** — `TextLayer` holds a fixed-capacity pool of per-line cosmic-text buffers (see `MAX_VISIBLE_ROW_SLOTS` in `editor-render`) so vertical resize does not realloc `Vec` backing storage after the first warm-up frame.
3. **Layout cache** — Document lines use a fixed large layout width (`editor-render` `TextLayer`) so horizontal resize does **not** invalidate cosmic-text shaping for visible rows; only edits, scale factor, or vertical range changes do.
4. **Windows modal resize loop** — While the user drags an edge, the OS may not dispatch a normal idle loop; the app paints from `WindowEvent::Resized` / `ScaleFactorChanged` synchronously so content keeps up.
5. **DPI** — `ScaleFactorChanged` triggers glyph re-rasterization at the new density; expect a slightly heavier prepare phase for one frame.

### Resize artifact catalog (M12 audit)

Baseline issues (still useful when regressing):

| Artifact | Typical cause | Mitigation in tree |
|----------|---------------|-------------------|
| Frozen / rubber-band content while dragging an edge (Windows) | Modal resize loop: redraws not tied to `Resized` | `editor-app` calls `paint_frame` synchronously from `WindowEvent::Resized` and `ScaleFactorChanged` (`PaintCause::Interactive`), not only from `RedrawRequested`. |
| Full visible-line reshape every horizontal resize | Layout width tied to viewport | Fixed large doc line width + shape key includes layout-width bits; horizontal resize does not reshaping visible lines. |
| `Vec` growth when height changes | Growing per-line buffer pools on demand | `TextLayer` pre-grows to `MAX_VISIBLE_ROW_SLOTS` after warm-up; see `gpu_resize_stress` test `resize_with_paint_does_not_grow_visible_row_scratch`. |
| Wrong vsync / latency on mixed refresh | Single present mode everywhere | `GpuContext::sync_present_mode_for_window` + refresh-rate-aware `choose_present_mode` (`editor-render` `gpu.rs`). |
| 120 Hz laptop on battery | Uncapped redraw cadence | ~30 s `battery` poll; on battery + high-refresh display, throttle `RedrawRequested` frames unless `power_uncap_on_battery` (M10 persistence). |
| Flash or wrong size at startup | `set_visible` before first present | Window created hidden; `set_visible(true)` after first successful `paint_frame`. |

**Latency during resize:** use `--latency-trace` for input-to-present on typing; for resize, use `--resize-telemetry` and `RUST_LOG=editor_app::resize_telemetry=info`. Expect occasional p99 > 16 ms during swapchain reconfigure; sustained frames > 100 ms warrant investigation.

**Automated GPU checks:** `cargo test -p editor-render --test gpu_resize_stress` (integration; needs GPU/window when the runner allows — see CI job `m12-gpu-resize-windows`). Tests skip gracefully when init fails.

**Telemetry:** run `editor-app --resize-telemetry` (see [`scripts/resize-stress.ps1`](../scripts/resize-stress.ps1) and [`scripts/resize-stress.sh`](../scripts/resize-stress.sh)).

---

*See also `PERFORMANCE_BUDGETS.md`, `TESTING_STRATEGY.md`, `MVP_ACCEPTANCE.md`.*
