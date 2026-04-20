# Changelog

All notable changes to this project are documented here. The format is based on
[Keep A Changelog](https://keepachangelog.com/en/1.1.0/) and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) once it
reaches a tagged release.

## [Unreleased]

### Added
- **M01: Repo Scaffolding, Workspace, Toolchain, CI.**
  - Virtual cargo workspace (`Cargo.toml`) with five member crates under
    `crates/`: `editor-core`, `editor-render`, `editor-input`, `editor-io`,
    and the binary crate `editor-app` (produces the `ide` executable).
  - Per-crate stubs with passing unit tests (one per library crate; 5 tests
    total).
  - Toolchain pinned to Rust 1.94.1 via `rust-toolchain.toml`.
  - `rustfmt.toml` with stable-only options.
  - Shared `[workspace.lints]` table (Rust + Clippy) with hot-path-unfriendly
    lints (`dbg_macro`, `todo`, `unimplemented`, `print_stderr`,
    `print_stdout`) treated as warnings.
  - Release profile: `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`,
    `panic = "abort"`. Bench profile inherits with debug symbols.
  - `deny.toml` for `cargo-deny` (licenses, advisories, bans, sources)
    with `allow-wildcard-paths = true` for internal workspace path
    dependencies and `unused-allowed-license = "allow"` to keep the
    forward-looking license allowlist quiet.
  - GitHub Actions CI matrix (Windows / Linux / macOS) running `cargo fmt`,
    `cargo clippy -D warnings`, `cargo test`, and `cargo build --release`.
  - Separate rustdoc job with intra-doc link checking.
  - Blocking `cargo-deny` job (licenses / advisories / bans / sources).
  - Dependabot for cargo + GitHub Actions on a weekly cadence.
  - Root `FOLLOWUPS.md` for deferred work items.
- **Corrections to `TECH_STACK.md`.** Version pins updated to reflect
  April-2026 stable releases: `wgpu 29` (was `23`), `glyphon 0.11` (was
  `0.6`), `winit 0.30` (clarified that `0.31` is in beta). Added an explicit
  version policy stating that deps are only added to `Cargo.toml` when a
  mission actually adopts them, to avoid pinning stale versions.

### Added — M00 (previous)
- **M00: Foundation Research & Documentation.** Initial commit of repository
  scaffolding, licensing (`LICENSE-APACHE`, `LICENSE-MIT`), project `README.md`,
  root-level `ARCHITECTURE.md` and `TECH_STACK.md`, and the full `docs/` reference
  tree covering performance model, text engine, rendering pipeline, input
  pipeline, concurrency, file I/O, cross-platform strategy, observability,
  testing, risks, glossary, external references, mission index, and status
  tracking. Git repository initialized on the `main` branch with remote
  pointing at `https://github.com/HiNala/IDE.git`.
