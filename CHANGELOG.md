# Changelog

All notable changes to this project are documented here. The format is based on
[Keep A Changelog](https://keepachangelog.com/en/1.1.0/) and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) once it
reaches a tagged release.

## [Unreleased]

### Added
- **M01: Repo Scaffolding, Workspace, Toolchain, CI.**
  - Virtual workspace with **six** crates: `editor-core`, `editor-input`,
    `editor-render`, `editor-io`, `editor-ui`, and `editor-app` (`editor-app`
    binary).
  - `editor-app` opens a `winit` window and clears the framebuffer via
    `wgpu` (`GpuContext` in `editor-render`); `--dry-run` runs headless
    adapter/device init for CI without a display server.
  - `CoreError` / `CoreResult` in `editor-core`; Criterion bench harness
    (`rope_benches`) compile-checks in CI.
  - Toolchain pinned to Rust 1.94.1 via `rust-toolchain.toml` (+ `rust-src`,
    cross targets).
  - `.clippy.toml`, `.editorconfig`, optional `.githooks/pre-commit`.
  - Windows `winres` manifest embed (`longPathAware`, UTF-8 active code page).
  - `deny.toml` + GitHub Actions: `ci.yml`, `audit.yml` (`cargo-deny` +
    `cargo-audit`), `bench.yml` (compile-check benches).

### Changed
- `docs/` canonical set aligned with M00: breadcrumbs, `RUST_CONVENTIONS.md`,
  frozen PRDs under `reference/`, root `CONTRIBUTING.md` / `DEVELOPMENT.md` /
  `LICENSE`.

### Added — M00 (previous)
- **M00: Foundation Research & Documentation.** Initial commit of repository
  scaffolding, licensing (`LICENSE-APACHE`, `LICENSE-MIT`), project `README.md`,
  root-level `ARCHITECTURE.md` and `TECH_STACK.md`, and the full `docs/` reference
  tree covering performance model, text engine, rendering pipeline, input
  pipeline, concurrency, file I/O, cross-platform strategy, observability,
  testing, risks, glossary, external references, mission index, and status
  tracking. Git repository initialized on the `main` branch with remote
  pointing at `https://github.com/HiNala/IDE.git`.
