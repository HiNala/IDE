[← docs/](./) · [README](../README.md)

# Architecture (summary)

## Crate graph

```text
editor-app   (binary, winit event loop, wires crates)
    ├── editor-render   (wgpu + surface; GPU only)
    ├── editor-input    (winit → editor commands)
    ├── editor-ui       (layout chrome; no GPU/winit)
    ├── editor-io       (disk I/O only)
    └── editor-core     (pure text engine; no I/O, no GPU)

editor-render → editor-core (types only)
editor-input   → editor-core
editor-ui      → editor-core
editor-io      → editor-core
```

**Rule:** `editor-core` must not depend on `winit`, `wgpu`, or the filesystem.

## Current implementation snapshot

- **editor-core:** errors, scroll offset, `word_nav` (Unicode word boundaries). Full rope `Document` is planned in M02.
- **editor-render:** `GpuContext` — swapchain, clear, present; `dry_run_headless` for CI.
- **editor-app:** window + `render_clear` loop; `--dry-run` for headless init.

## Missions

Authoritative numbered mission briefs: [`docs/missions/`](missions/). Short index: [`MISSIONS.md`](MISSIONS.md).
