# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `editor-core`: `TextBuffer` (rope-backed, LF-internal, byte-centric API), `LineEnding`, `Edit` / `EditKind`, `UndoStack`, `Cursor` / `CursorMotion`, `Selection`, `BytePos` / `LineCol`.
- `editor-core`: Criterion benchmarks for insert, line queries, snapshots; proptest integration tests for buffer invariants.

### Fixed

- Restored empty/corrupt root `Cargo.toml` and `editor-app` `main.rs` so the workspace builds again.
- `editor-core`: `word_nav` module with Unicode word-boundary navigation and unit tests.

### Changed

- `editor-app`: allow `clippy::print_stderr` for CLI help and error output on stderr.
- `editor-core`: extended `CoreError` with invalid char boundary, column, and range variants.

## [0.1.0-mvp] - 2026-04-20

Pre-alpha MVP scaffold: multi-crate workspace, `winit` + `wgpu` hello window, headless `--dry-run` GPU smoke test.
