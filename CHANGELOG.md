# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `editor-app` (M13 + M14): wired `editor-workspace::{Workspace, BufferManager}` and `editor-ui::{Sidebar, TabStrip, QuickOpenPalette}` into the window loop. Folder CLI arg (`editor-app <dir>`) opens the tree; `Ctrl+B` / `Ctrl+Shift+E` toggle/focus the sidebar; `Ctrl+P` opens fuzzy quick-open; `Ctrl+Tab` / `Ctrl+Shift+Tab` cycle buffers; `Ctrl+W` closes, `Ctrl+N` creates a scratch buffer. Mouse routing respects chrome zones; workspace FS events flag `external_modified` on matching buffers; sidebar visibility + width persist across sessions.
- `editor-app` (M18 light): status bar shows the current `git` branch via `editor-git::GitRepo::discover`, refreshed every 5 s and on workspace open.
- `editor-workspace`: `configure_walk` now threads `include_ide_internals` so `WalkOptions::include_ide_internals` actually exposes `.ide/**` paths to the walker instead of being overridden by a blanket exclude. Lifts the previous test failure for `walk_hides_dot_ide_by_default`.
- CI: **`m12-gpu-resize-windows`** runs `cargo test -p editor-render --test gpu_resize_stress` on Windows (M12).
- Docs: `docs/DIAGNOSING_PERFORMANCE.md` resize **artifact catalog** + telemetry notes; restored `scripts/resize-stress.sh`; `docs/STATUS.md` / `MISSION_IMPLEMENTATION_STATUS.md` M12 status.
- `editor-render` (M04 follow-up): Criterion bench `text_layer_prepare` (~10 MiB buffer, 1080p viewport; writes baseline with `--save-baseline m04-mvp`); ignored integration test `tests/visual_smoke.rs`; bundled-font smoke unit test; docs: default JetBrains Mono policy in `RENDERING_PIPELINE.md` / `TECH_STACK.md`.
- `editor-io` (M06): `LoadProgress`, `load_file_async` / `save_file_async` (WorkerPool + `crossbeam-channel`); `LoadedFile::line_ending`; cooperative load cancellation; save fault-injection unit test for atomic writes; docs updates (`TEXT_ENGINE`, `CROSS_PLATFORM`, `ARCHITECTURE`).
- `editor-app` (M06): polls `LoadProgress` (including load % in title when available), cancels superseded loads, resets buffer on failed/cancelled load without dropping the last tab.
- `editor-input` (M05): [`map_keyboard_input`](crates/editor-input/src/lib.rs) for unit tests and benches without constructing `winit::event::KeyEvent`; Criterion bench `translate`.
- `editor-app` (M05): Criterion bench `input_hotpath` (keyboard map + `TextBuffer::insert` CPU proxy); `--latency-trace` and frame budget warnings documented in [`INPUT_AND_IME.md`](docs/INPUT_AND_IME.md) / [`PERFORMANCE_BUDGETS.md`](docs/PERFORMANCE_BUDGETS.md).
- `editor-render` (M03): `backend` module for platform `wgpu::Backends` defaults, `FrameTimer` + `EditorRenderer::frame_timer`, `IDE_PRESENT_MODE` / `IDE_POWER_PREFERENCE`, resilient `acquire_present_surface_texture`, `EditorRenderer::{gpu, surface_format, surface_config}`; Criterion bench `frame_overhead`.
- `editor-app` (M03): animated per-frame clear color; `IDE_STATIC_CLEAR=1` restores solid `#1e1e1e` background for reading.
- `editor-render`: pre-grow cosmic-text visible-line scratch to `MAX_VISIBLE_ROW_SLOTS` (M12) so resize does not realloc row `Vec`s after warm-up; integration test `resize_with_paint_does_not_grow_visible_row_scratch`.
- Docs: resize diagnostics note row-scratch pooling (`DIAGNOSING_PERFORMANCE.md`, `RENDERING_PIPELINE.md`).
- Docs: `TEXT_ENGINE.md` aligned with implemented `editor-core` API (`TextBuffer`, `UndoStack`, etc.); crate-level `editor-core` doctest shows insert → undo → redo.
- Docs: README / `ARCHITECTURE.md` list all workspace crates (`editor-diff`, `editor-workspace` beyond original M01 six-pack); `editor-core::CoreError` doctest for M01 error-convention checklist.
- CI: `release.yml` — on `v*` tags, build `editor-app` for Windows/Linux/macOS, attach artifacts and `SHA256SUMS.txt` to GitHub Releases (M11 partial).
- `Cargo.toml`: `[profile.release]` sets `incremental = false`; `[profile.release-with-debug]` for profiling (M11 doc alignment).
- `editor-core`: `TextBuffer` (rope-backed, LF-internal, byte-centric API), `LineEnding`, `Edit` / `EditKind`, `UndoStack`, `Cursor` / `CursorMotion`, `Selection`, `BytePos` / `LineCol`, `WorkerPool` for background jobs.
- `editor-core`: Criterion benchmarks for insert, line queries, snapshots, incoherent inserts, deletes, cursor motion, undo checkpoint; proptest at **256 cases** per block (cursor boundary, undo/redo round-trip).
- Docs: [`API_DESIGN_NOTES.md`](docs/API_DESIGN_NOTES.md) — pointer to the implemented public API.
- `editor-core`: `word_nav` — Unicode word-boundary navigation and deletion ranges.
- `editor-render`: `EditorRenderer` + glyphon text pipeline, gutter, status bar layout integration.
- `editor-app`: typing, navigation, selection, clipboard (`arboard`), undo/redo, save/load (sync on worker + channel), session persistence (`state.json`), file dialogs (`rfd`).
- Docs: [`docs/MISSION_IMPLEMENTATION_STATUS.md`](docs/MISSION_IMPLEMENTATION_STATUS.md), [`docs/RELEASING.md`](docs/RELEASING.md).

### Fixed

- Restored empty/corrupt root `Cargo.toml` and `editor-app` `main.rs` so the workspace builds again.

### Changed

- `editor-app`: allow `clippy::print_stderr` for CLI help and error output on stderr.
- `editor-core`: extended `CoreError` with invalid char boundary, column, and range variants.

## [0.1.0-mvp] - 2026-04-20

Pre-alpha MVP scaffold: multi-crate workspace, `winit` + `wgpu` hello window, headless `--dry-run` GPU smoke test.
