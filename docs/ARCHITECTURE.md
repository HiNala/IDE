[← docs/](./) · [README](../README.md)

# Architecture (current)

## Crate graph

```text
editor-app   (binary: winit loop, session persistence, I/O coordination)
    ├── editor-render   (wgpu + glyphon; text, gutter, cursor, status bar)
    ├── editor-input    (winit → EditorCommand)
    ├── editor-ui       (layout: status bar, quick-open, find/replace + search panel chrome)
    ├── editor-io       (disk: load, encode, atomic save)
    ├── editor-search   (in-file + project-wide search; `regex` + `grep-searcher` + `ignore`)
    ├── editor-diff     (line/char diff utilities — M17)
    ├── editor-workspace (workspace root, BufferManager, `notify` watcher — M13)
    └── editor-core     (rope TextBuffer, cursor, selection, undo, word nav, WorkerPool)

editor-workspace
    ├── editor-core
    └── editor-io

editor-ai-tools (M20 agent tools + `WorkspaceTx`; optional M27 skill tools via `editor-skills`)
    ├── editor-ai-provider (`ToolDef` / `ChatRequest`)
    ├── editor-core, editor-diff, editor-io, editor-search, editor-workspace
    └── editor-skills (optional)

editor-skills (M27: `SKILL.md` registry; used by chat when wired)

editor-diff (line/char diff utilities; used by future diff UI — M17)
    └── editor-core

editor-search
    ├── editor-core
    └── editor-workspace

editor-render → editor-core, editor-ui
editor-input   → editor-core
editor-ui      → editor-core, editor-search
editor-io      → editor-core
```

**Rules**

- **`editor-core`** must not depend on `winit`, `wgpu`, or the filesystem. It may use threads (`WorkerPool`, `crossbeam-channel`) for blocking work off the UI path.
- **`editor-io`** is the only crate that performs file I/O to user paths (except `editor-app` writing `state.json` under the config directory).
- **`editor-render`** owns the only `wgpu::Device` in the process. `GpuContext` wraps instance /
  adapter / surface / swapchain; `EditorRenderer` composes text + quads and owns a `FrameTimer`
  for wall-clock frame intervals. Backend bitmask selection lives in `crates/editor-render/src/backend.rs`.

## Runtime model

- **Main thread:** `winit` event loop, `EditorRenderer::render` (GPU + glyphon), input handling, `poll_io` for completed background tasks.
- **Worker pool:** `editor_core::WorkerPool` runs file load/save closures; results return via `crossbeam_channel` to `try_recv` on the main thread (no blocking wait in the frame loop). Loads use `editor_io::load_file_async` (progress + cancellation); saves use `editor_io::save_file_async`; both integrate in `editor-app::poll_io`.
- **Blink timer:** A separate thread sends `AppEvent::BlinkTick` via `EventLoopProxy` (~2 Hz) for caret blink.

### Frame loop phases (`editor-app`)

On `RedrawRequested`, `paint_frame`:

1. **Prepare** — text layer prepare / layout work used for the frame (`MetricsCollector` records duration).
2. **GPU** — acquire swapchain texture, encode passes, submit queue, present (recorded separately).
3. **Totals** — `prepare + gpu` compared to M05 budgets (warn via `tracing`, target `editor_app::frame`; stricter in dev builds).
4. **IME** — after a successful present, `Window::set_ime_cursor_area` updates from caret geometry + scroll.

Per-frame stats feed the dev HUD (Ctrl+F11); periodic p50/p95/p99 summaries are logged at debug level from `metrics`.

## Frame input

`editor_render::FrameInput` carries:

- Text buffer snapshot, scroll, cursor, selection, scale factor, status bar, dev HUD, quick-open / M16 chrome text, find-bar backdrop height, in-file search highlight ranges, optional diff paint.

## Workspace and buffers (M13)

See [`WORKSPACE_MODEL.md`](WORKSPACE_MODEL.md): `Workspace` (root, ignore rules, `notify`, file walk) and `BufferManager` (multiple `TextBuffer` tabs, MRU order). The app loop polls filesystem events and updates per-buffer external-modified flags.

## Persistence

Per **M10**, `editor-app` reads/writes `state.json` (via `directories` + serde JSON) for last file, cursor byte, scroll, window geometry. Atomic write pattern avoids corrupt state on crash.

## Missions

Authoritative briefs: [`docs/missions/`](missions/). Implementation status: [`MISSION_IMPLEMENTATION_STATUS.md`](MISSION_IMPLEMENTATION_STATUS.md).

*Last updated: 2026-04-20.*
