[← docs/](./) · [README](../README.md)

# Architecture (current)

## Crate graph

```text
editor-app   (binary: winit loop, session persistence, I/O coordination)
    ├── editor-render   (wgpu + glyphon; text, gutter, cursor, status bar)
    ├── editor-input    (winit → EditorCommand)
    ├── editor-ui       (layout: status bar strings; no GPU)
    ├── editor-io       (disk: load, encode, atomic save)
    └── editor-core     (rope TextBuffer, cursor, selection, undo, word nav, WorkerPool)

editor-render → editor-core, editor-ui
editor-input   → editor-core
editor-ui      → editor-core (types only)
editor-io      → editor-core
```

**Rules**

- **`editor-core`** must not depend on `winit`, `wgpu`, or the filesystem. It may use threads (`WorkerPool`, `crossbeam-channel`) for blocking work off the UI path.
- **`editor-io`** is the only crate that performs file I/O to user paths (except `editor-app` writing `state.json` under the config directory).
- **`editor-render`** owns the only `wgpu::Device` in the process.

## Runtime model

- **Main thread:** `winit` event loop, `EditorRenderer::render` (GPU + glyphon), input handling, `poll_io` for completed background tasks.
- **Worker pool:** `editor_core::WorkerPool` runs file load/save closures; results return via `crossbeam_channel` to `try_recv` on the main thread (no blocking wait in the frame loop).
- **Blink timer:** A separate thread sends `AppEvent::BlinkTick` via `EventLoopProxy` (~2 Hz) for caret blink.

## Frame input

`editor_render::FrameInput` carries:

- Text buffer snapshot (or equivalent), scroll, cursor, selection, scale factor, optional line gutter, optional status bar info, dev HUD flag.

## Persistence

Per **M10**, `editor-app` reads/writes `state.json` (via `directories` + serde JSON) for last file, cursor byte, scroll, window geometry. Atomic write pattern avoids corrupt state on crash.

## Missions

Authoritative briefs: [`docs/missions/`](missions/). Implementation status: [`MISSION_IMPLEMENTATION_STATUS.md`](MISSION_IMPLEMENTATION_STATUS.md).

*Last updated: 2026-04-20.*
