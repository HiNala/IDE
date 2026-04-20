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

---

*See also `PERFORMANCE_BUDGETS.md`, `TESTING_STRATEGY.md`, `MVP_ACCEPTANCE.md`.*
