# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- CI: `release.yml` — on `v*` tags, build `editor-app` for Windows/Linux/macOS, attach artifacts and `SHA256SUMS.txt` to GitHub Releases (M11 partial).
- `Cargo.toml`: `[profile.release]` sets `incremental = false`; `[profile.release-with-debug]` for profiling (M11 doc alignment).
- `editor-core`: `TextBuffer` (rope-backed, LF-internal, byte-centric API), `LineEnding`, `Edit` / `EditKind`, `UndoStack`, `Cursor` / `CursorMotion`, `Selection`, `BytePos` / `LineCol`, `WorkerPool` for background jobs.
- `editor-core`: Criterion benchmarks for insert, line queries, snapshots; proptest integration tests for buffer invariants.
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
