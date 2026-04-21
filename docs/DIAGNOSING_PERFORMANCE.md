[← docs/](./) · [README](../README.md)

# Diagnosing performance

Cookbook for when the editor feels slow or stutters. Expanded in **M07** (observability) and **M08** (acceptance).

## Quick checks

1. **Dev HUD / metrics** — When implemented (M07), toggle the overlay (e.g. F11) and read p50/p95/p99 frame times and RSS.
2. **Tracing** — `RUST_LOG=editor_app=debug,editor_render=debug` for frame and subsystem spans.
3. **Tracy** — Optional `--features tracy` build; connect the Tracy viewer for flamegraphs.

## Symptom → action

| Symptom | What to check |
|---------|----------------|
| Feels laggy but HUD p99 is fine | Input path vs render path; enable trace on `editor-input`. |
| One large file is slow | Load path (mmap vs stream), atlas/glyph warnings in `editor_render` (M04+). |
| Open is slow | Disk throughput; whether mmap succeeded (logs in `editor_io`, M06+). |
| Scrolling stutters | Tracy timeline; GPU submit vs prepare split. |

## GPU device lost

If `wgpu` reports device loss, the app should attempt one recovery (M08 polish). If recovery fails, exit code **2** is reserved (see `DEVELOPMENT.md`). Until implemented, check logs for `ERROR` from `editor_render`.

## Resize (M12)

Typical costs during window resize:

1. **Swapchain reconfiguration** — `GpuContext::resize` updates `SurfaceConfiguration` and calls `surface.configure`. The swapchain is recreated by the driver; keep document shaping out of this path where possible.
2. **Layout cache** — Document lines use a fixed large layout width (`editor-render` `TextLayer`) so horizontal resize does **not** invalidate cosmic-text shaping for visible rows; only edits, scale factor, or vertical range changes do.
3. **Windows modal resize loop** — While the user drags an edge, the OS may not dispatch a normal idle loop; the app paints from `WindowEvent::Resized` / `ScaleFactorChanged` synchronously so content keeps up.
4. **DPI** — `ScaleFactorChanged` triggers glyph re-rasterization at the new density; expect a slightly heavier prepare phase for one frame.

**Telemetry:** run `editor-app --resize-telemetry` and set `RUST_LOG=editor_app::resize_telemetry=info` to log each resize frame duration (see `scripts/resize-stress.ps1`).

---

*See also `PERFORMANCE_BUDGETS.md`, `TESTING_STRATEGY.md`, `MVP_ACCEPTANCE.md`.*
