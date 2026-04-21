# M01 — Repo Scaffolding, Cargo Workspace, Toolchain, CI

**Mission ID:** M01
**Prerequisites:** M00 complete (docs and reference folders exist, repo is initialized).
**Output:** A multi-crate Cargo workspace with a working "Hello, window" smoke test, green CI on Windows/Linux/macOS, pinned toolchain, clippy + fmt + deny checks enforced.
**Estimated scope:** 1 session.

---

## Read First

Before starting this mission, re-read:

- `/00_MISSION_INDEX.md` — standing orders.
- `/docs/TECH_STACK.md` — dependency choices.
- `/docs/ARCHITECTURE.md` — especially the crate layout section.
- `/docs/RUST_CONVENTIONS.md` — style, errors, logging conventions.
- `/docs/CROSS_PLATFORM.md` — what we need to set up for Windows, Linux, and macOS.
- `/reference/01_TECH_STACK.md` — the PRD's tech stack supporting doc.

Search the web for current best practices if anything in the docs looks stale. Specifically: check the latest stable Rust version (via `rustup`), confirm `winit`, `wgpu`, and `ropey` current versions, and confirm that `actions-rust-lang/setup-rust-toolchain` is still the recommended GitHub Action (as of early 2026 it is).

---

## The Situation In Plain English

M00 gave us a `/docs` folder and an empty repo. M01 turns that empty repo into a real, buildable Rust project with every guardrail up: a multi-crate workspace mirroring the architecture we documented, a pinned stable Rust toolchain, formatting and linting enforced, CI running on all three target OSes, and a minimal but meaningful "Hello, window" smoke test that proves `winit` and `wgpu` can open a window and clear the screen on Windows, Linux, and macOS. No text rendering yet (that's M04), no rope (M02), no file I/O (M06). Just the foundation: workspace, deps, window, clear color.

The key insight driving the crate split is that the text engine must not know about GPUs and the render engine must not know about OS events. This one-way dependency discipline is what keeps the editor fast at scale. We will enforce it through crate boundaries: `editor-core` has zero `winit` or `wgpu` deps, `editor-input` depends on `winit` and `editor-core`, `editor-render` depends on `wgpu` and `editor-core`, and so on. Circular deps are impossible.

We also set up CI carefully. CI is not a nice-to-have; it is the safety net that lets agents ship aggressive changes without breaking cross-platform support. The CI matrix runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, and `cargo build --release` on Windows, Ubuntu, and macOS. A failing CI job on any OS fails the mission.

The user develops primarily on Windows. Windows-specific setup (Visual Studio Build Tools, MSVC toolchain, long path support, UTF-8 codepage) needs to work out of the box with a short, well-documented setup path. Don't assume anything about the developer's machine.

---

## Scope

**In scope:**
- Cargo workspace with six crates: `editor-core`, `editor-input`, `editor-render`, `editor-io`, `editor-ui`, `editor-app`.
- `rust-toolchain.toml` pinning the stable Rust version.
- `rustfmt.toml` and `.clippy.toml` with project-wide settings.
- `deny.toml` (cargo-deny) for license and supply-chain checks.
- `.editorconfig`.
- GitHub Actions workflows: `ci.yml` (build, test, clippy, fmt on three OSes), `bench.yml` (runs on push to main), `audit.yml` (security audit via `cargo-audit`).
- A "Hello, window" smoke test in `editor-app`: opens a winit window, initializes wgpu, renders a solid clear color every frame, exits cleanly on close. Runs on all three OSes.
- Updates to `docs/STATUS.md` and `CHANGELOG.md`.

**Out of scope:**
- Text rendering (M04).
- Rope / text buffer (M02).
- File I/O (M06).
- Input handling beyond close-window (M05).
- Any UI elements (M04/M09).
- Release packaging (M11).

---

## North Star For This Mission

At the end of M01, a developer on Windows, Linux, or macOS can:

1. Clone https://github.com/HiNala/IDE.
2. Install Rust if they don't have it (documented in `DEVELOPMENT.md`).
3. Run `cargo run --release --bin editor-app`.
4. See a blank window with a dark background.
5. Close the window; the process exits cleanly.

And CI runs green on every push to every branch.

---

## TODO List

### 1. Pin the Rust toolchain

- [ ] 1.1. Check the current stable Rust version via `rustup` (web search: "rust stable version 2026" if unsure). Create `/rust-toolchain.toml`:
  ```toml
  [toolchain]
  channel = "1.85"  # or whatever the current stable is — pin an exact minor version
  components = ["rustfmt", "clippy", "rust-src"]
  targets = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "aarch64-apple-darwin"]
  profile = "default"
  ```
- [ ] 1.2. Pin the version deliberately. Do not pin to a nightly. Do not use `stable` as the channel — that drifts. Use the exact current stable number. Update `/docs/TECH_STACK.md` and `/DEVELOPMENT.md` to match.
- [ ] 1.3. Commit: `build(toolchain): pin Rust stable X.YY.Z and components`.

### 2. Create the Cargo workspace

- [ ] 2.1. Create the root `/Cargo.toml` as a workspace manifest:
  ```toml
  [workspace]
  resolver = "2"
  members = [
    "crates/editor-core",
    "crates/editor-input",
    "crates/editor-render",
    "crates/editor-io",
    "crates/editor-ui",
    "crates/editor-app",
  ]

  [workspace.package]
  version = "0.0.1"
  edition = "2021"
  license = "MIT OR Apache-2.0"
  repository = "https://github.com/HiNala/IDE"
  rust-version = "1.85"  # match rust-toolchain.toml

  [workspace.dependencies]
  # Centralize versions here; members inherit via `x.workspace = true`
  winit = "0.30"
  wgpu = "24"  # or the current stable major — verify via web_search before pinning
  pollster = "0.4"
  ropey = "1.6"
  thiserror = "2"
  anyhow = "1"
  tracing = "0.1"
  tracing-subscriber = { version = "0.3", features = ["env-filter"] }
  memmap2 = "0.9"
  arboard = "3"
  parking_lot = "0.12"
  bytemuck = { version = "1", features = ["derive"] }
  raw-window-handle = "0.6"
  # dev-deps
  criterion = { version = "0.5", features = ["html_reports"] }
  proptest = "1"
  insta = "1"

  [profile.release]
  opt-level = 3
  lto = "thin"
  codegen-units = 1
  debug = false
  strip = "symbols"
  panic = "abort"

  [profile.release-dev]
  inherits = "release"
  debug = true
  strip = "none"

  [profile.bench]
  opt-level = 3
  lto = "thin"
  debug = true
  ```
- [ ] 2.2. **Verify current stable versions** for all listed deps by searching the web or visiting `docs.rs/<crate>`. Pin minor versions where stable (`winit = "0.30"`), pin major where aggressive version churn is expected (`wgpu` — pin current major). Document the chosen versions in `/docs/TECH_STACK.md`.
- [ ] 2.3. Create the directory `crates/` at the repo root and the six member directories under it, each with their own stub `Cargo.toml` and `src/lib.rs` (or `src/main.rs` for `editor-app`).
- [ ] 2.4. Commit: `build(workspace): create Cargo workspace with six member crates`.

### 3. Populate each member crate's `Cargo.toml`

- [ ] 3.1. **`crates/editor-core/Cargo.toml`** — the pure logic crate:
  ```toml
  [package]
  name = "editor-core"
  version.workspace = true
  edition.workspace = true
  license.workspace = true
  repository.workspace = true
  rust-version.workspace = true

  [dependencies]
  ropey.workspace = true
  thiserror.workspace = true
  tracing.workspace = true

  [dev-dependencies]
  criterion.workspace = true
  proptest.workspace = true

  [[bench]]
  name = "rope_benches"
  harness = false
  ```
  (The actual bench file comes in M02. The `[[bench]]` entry creates the expectation.)
- [ ] 3.2. **`crates/editor-input/Cargo.toml`** — depends on `editor-core` and `winit`. No GPU, no I/O.
- [ ] 3.3. **`crates/editor-render/Cargo.toml`** — depends on `editor-core`, `wgpu`, `raw-window-handle`, `bytemuck`, `pollster` (for the synchronous init path in dev/test binaries). `glyphon` will be added in M04; do not add it here yet.
- [ ] 3.4. **`crates/editor-io/Cargo.toml`** — depends on `editor-core`, `memmap2`, `thiserror`, `tracing`.
- [ ] 3.5. **`crates/editor-ui/Cargo.toml`** — depends on `editor-core` only for now; later missions add render and input deps.
- [ ] 3.6. **`crates/editor-app/Cargo.toml`** — the top-level binary, depends on all the above plus `winit`, `wgpu`, `anyhow`, `tracing`, `tracing-subscriber`, `pollster`. This is where `main` lives.
  ```toml
  [[bin]]
  name = "editor-app"
  path = "src/main.rs"
  ```
- [ ] 3.7. Each crate's `src/lib.rs` (or `main.rs` for `editor-app`) should contain at minimum:
  ```rust
  //! <crate-name>
  //!
  //! <one-line purpose from docs/ARCHITECTURE.md>

  #![warn(clippy::all, clippy::pedantic)]
  #![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]
  ```
- [ ] 3.8. Run `cargo build --workspace` and confirm every member crate compiles as an empty stub.
- [ ] 3.9. Commit: `build(crates): scaffold six workspace members with per-crate Cargo.toml`.

### 4. Set up formatting and linting

- [ ] 4.1. Create `/rustfmt.toml`:
  ```toml
  edition = "2021"
  max_width = 100
  use_small_heuristics = "Max"
  imports_granularity = "Module"
  group_imports = "StdExternalCrate"
  reorder_imports = true
  newline_style = "Unix"
  ```
- [ ] 4.2. Create `/.clippy.toml` — keep minimal, we rely on `-D warnings` plus per-crate lint allowances in lib.rs/main.rs.
- [ ] 4.3. Create `/.editorconfig`:
  ```
  root = true

  [*]
  end_of_line = lf
  insert_final_newline = true
  charset = utf-8
  trim_trailing_whitespace = true
  indent_style = space
  indent_size = 4

  [*.{md,yml,yaml,toml}]
  indent_size = 2
  ```
- [ ] 4.4. Run `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings`. Fix any issues on the stub code.
- [ ] 4.5. Commit: `build(lint): add rustfmt, clippy, and editorconfig settings`.

### 5. Set up cargo-deny for supply-chain and license checks

- [ ] 5.1. Install `cargo-deny` locally. Document the install step in `/DEVELOPMENT.md`.
- [ ] 5.2. Create `/deny.toml` with sane defaults:
  - `[licenses]` — allow MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-DFS-2016, CC0-1.0, MPL-2.0. Deny copyleft like GPL unless we intentionally accept it. (Zed is GPL; our editor can be MIT/Apache-dual if we keep strictly non-GPL deps.)
  - `[bans]` — deny known duplicates, warn on multiple versions of the same crate.
  - `[advisories]` — deny any crate with an open CVE.
  - `[sources]` — only allow crates from crates.io (no unvetted git deps unless explicitly allowed).
- [ ] 5.3. Run `cargo deny check` locally and fix any issues.
- [ ] 5.4. Commit: `build(deny): add cargo-deny configuration for license and supply-chain checks`.

### 6. Write the "Hello, window" smoke test in `editor-app`

- [ ] 6.1. `crates/editor-app/src/main.rs` opens a winit window and renders a clear color via wgpu. Use the winit 0.30 `ApplicationHandler` trait pattern:
  ```rust
  use anyhow::Result;
  use winit::application::ApplicationHandler;
  use winit::event::WindowEvent;
  use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
  use winit::window::{Window, WindowId};

  struct App {
      window: Option<std::sync::Arc<Window>>,
      gpu: Option<editor_render::GpuContext>,
  }
  // ... (implement ApplicationHandler::resumed to create window + GpuContext,
  //       window_event for CloseRequested + Resized + RedrawRequested)
  ```
- [ ] 6.2. `crates/editor-render/src/lib.rs` exposes a `GpuContext` that wraps `wgpu::Instance`, `Surface`, `Device`, `Queue`, `SurfaceConfiguration`. Provide `GpuContext::new(window: Arc<Window>)` and `GpuContext::render_clear(&mut self, color: wgpu::Color)`. Handle surface reconfigure on resize.
- [ ] 6.3. Use `pollster::block_on` in `main` to initialize wgpu synchronously. Document that this is acceptable in a top-level `main` but not in library code.
- [ ] 6.4. Initialize `tracing_subscriber` in `main` before anything else, with `EnvFilter::from_default_env().or(EnvFilter::new("info"))` so `RUST_LOG=editor_app=debug` works.
- [ ] 6.5. On Windows, request the DX12 backend with Vulkan as a fallback. On macOS, Metal. On Linux, Vulkan with OpenGL as fallback. Use `wgpu::Backends::all()` with `wgpu::util::backend_bits_from_env` so `WGPU_BACKEND=vulkan` works.
- [ ] 6.6. On first run on Windows, confirm the window opens without any console or terminal appearing alongside. If it does, add `#![windows_subsystem = "windows"]` to `main.rs` (but keep it conditional — we still want a console in dev builds for `println!` debugging).
- [ ] 6.7. The app must exit cleanly on `WindowEvent::CloseRequested` (call `event_loop.exit()`). The process must return `0`.
- [ ] 6.8. Run `cargo run --release --bin editor-app`. Close the window. Confirm clean exit. Repeat on every OS available to you (if you're running on Windows, definitely Windows; verify CI catches Linux and macOS via the matrix).
- [ ] 6.9. Commit: `feat(app): add Hello-window smoke test with winit + wgpu`.

### 7. Write the GitHub Actions CI workflow

- [ ] 7.1. Create `.github/workflows/ci.yml`. Structure:
  ```yaml
  name: CI

  on:
    push:
      branches: [main]
    pull_request:
      branches: [main]

  env:
    CARGO_TERM_COLOR: always
    RUSTFLAGS: -Dwarnings
    RUST_BACKTRACE: 1

  jobs:
    fmt:
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4
        - uses: actions-rust-lang/setup-rust-toolchain@v1
          with:
            toolchain: stable
            components: rustfmt
        - run: cargo fmt --all -- --check

    clippy:
      strategy:
        matrix:
          os: [ubuntu-latest, windows-latest, macos-latest]
      runs-on: ${{ matrix.os }}
      steps:
        - uses: actions/checkout@v4
        - uses: actions-rust-lang/setup-rust-toolchain@v1
          with:
            toolchain: stable
            components: clippy
        - name: Install Linux graphics deps
          if: matrix.os == 'ubuntu-latest'
          run: |
            sudo apt-get update
            sudo apt-get install -y libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxinerama-dev libxcursor-dev libxi-dev libgl1-mesa-dev libegl1-mesa-dev mesa-vulkan-drivers
        - run: cargo clippy --workspace --all-targets --all-features -- -D warnings

    test:
      strategy:
        matrix:
          os: [ubuntu-latest, windows-latest, macos-latest]
      runs-on: ${{ matrix.os }}
      steps:
        - uses: actions/checkout@v4
        - uses: actions-rust-lang/setup-rust-toolchain@v1
          with:
            toolchain: stable
        - name: Install Linux graphics deps
          if: matrix.os == 'ubuntu-latest'
          run: |
            sudo apt-get update
            sudo apt-get install -y libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxinerama-dev libxcursor-dev libxi-dev libgl1-mesa-dev libegl1-mesa-dev mesa-vulkan-drivers
        - run: cargo test --workspace --all-features
        - run: cargo build --release --workspace
  ```
- [ ] 7.2. Add `.github/workflows/audit.yml`:
  ```yaml
  name: Security Audit
  on:
    push:
      paths: [Cargo.toml, Cargo.lock, deny.toml, .github/workflows/audit.yml]
    schedule:
      - cron: "0 0 * * 1"  # weekly Monday
  jobs:
    audit:
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4
        - uses: actions-rust-lang/setup-rust-toolchain@v1
        - run: cargo install cargo-deny cargo-audit --locked
        - run: cargo deny check
        - run: cargo audit
  ```
- [ ] 7.3. Add `.github/workflows/bench.yml`:
  ```yaml
  name: Benchmarks
  on:
    push:
      branches: [main]
  jobs:
    bench:
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4
        - uses: actions-rust-lang/setup-rust-toolchain@v1
        - run: cargo bench --workspace --no-run  # compile benches to catch breakage
  ```
  (Actual bench runs and regression checks come in M07; for now we just compile-check.)
- [ ] 7.4. Push and watch CI. Fix anything red. Don't merge a failing CI workflow. On Windows runners, if `wgpu` can't find a GPU, tests that hit `GpuContext::new` should skip gracefully. Use `cfg(test)` feature-gating where needed.
- [ ] 7.5. **Linux CI specifically:** the `ubuntu-latest` runner lacks a display server for actually creating a window. Do not run the app binary in CI on Linux — just build and unit-test. Document this in the workflow comment.
- [ ] 7.6. Commit: `ci: add ci, audit, and bench workflows for Windows/Linux/macOS`.

### 8. Add a pre-commit hook (optional but recommended)

- [ ] 8.1. Create `.githooks/pre-commit` (bash, cross-platform via `sh`):
  ```bash
  #!/usr/bin/env sh
  set -e
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  ```
- [ ] 8.2. Document in `DEVELOPMENT.md` how to enable the hook: `git config core.hooksPath .githooks`.
- [ ] 8.3. Commit: `build(hooks): add optional pre-commit hook for fmt + clippy`.

### 9. Write a minimum README "Quick Start"

- [ ] 9.1. Extend `/README.md` (or the "Getting Started" section) to include the now-working quick start:
  ```
  git clone https://github.com/HiNala/IDE.git
  cd IDE
  cargo run --release --bin editor-app
  ```
  And a one-liner explaining what to expect (a blank dark window that exits cleanly on close).
- [ ] 9.2. Link to `DEVELOPMENT.md` for platform prerequisites (MSVC Build Tools on Windows, `libx11-dev` etc. on Linux, Xcode Command Line Tools on macOS).
- [ ] 9.3. Commit: `docs(readme): document quickstart after scaffolding`.

### 10. Enforce Windows long-path and UTF-8 codepage settings

- [ ] 10.1. Add a `<assembly>` manifest entry or a `build.rs` note in `editor-app` for Windows long path awareness and UTF-8 active code page. Reference: Microsoft's manifest documentation. (This matters because some users' repos live in paths > 260 chars, and CRT I/O defaults to system codepage which mangles non-ASCII filenames.)
- [ ] 10.2. Document this in `/docs/CROSS_PLATFORM.md` if not already there.
- [ ] 10.3. Commit: `feat(windows): enable long-path and UTF-8 codepage via app manifest`.

### 11. Add a minimal integration test

- [ ] 11.1. In `crates/editor-app/tests/smoke_test.rs`, write a test that ensures the binary exits cleanly when given `--dry-run` (which you will implement: the app parses args, and with `--dry-run`, it initializes the GPU, confirms it has a valid surface config, and exits with 0 without ever showing a window). This lets CI validate more than compilation on Linux without a display server... actually it still needs a display — revise: the `--dry-run` flag should initialize wgpu in headless mode (`wgpu::InstanceDescriptor` without a surface) and exit. Test that path.
- [ ] 11.2. Run `cargo test --workspace` and confirm pass on Windows. Confirm CI runs it on all three OSes.
- [ ] 11.3. Commit: `test(app): add --dry-run smoke test for headless GPU init`.

### 12. Initialize `editor-core`'s `Error` and `Result` types

- [ ] 12.1. Even though `editor-core` has almost no logic yet, establish the error convention now. In `crates/editor-core/src/error.rs`:
  ```rust
  use thiserror::Error;

  #[derive(Debug, Error)]
  pub enum CoreError {
      #[error("invalid byte offset {offset} in document of length {len}")]
      InvalidOffset { offset: usize, len: usize },

      #[error("invalid line index {line} in document with {total_lines} lines")]
      InvalidLineIndex { line: usize, total_lines: usize },
  }

  pub type CoreResult<T> = Result<T, CoreError>;
  ```
- [ ] 12.2. Export from `lib.rs`. Add a doctest.
- [ ] 12.3. Commit: `feat(core): establish CoreError and CoreResult types`.

### 13. Set up `tracing` properly

- [ ] 13.1. In `editor-app/src/main.rs`, initialize `tracing_subscriber` with:
  - An `EnvFilter` (so `RUST_LOG` works).
  - A compact format in release builds, a pretty format in debug builds (use `cfg!(debug_assertions)` to switch).
  - Optional Tracy layer behind a `"tracy"` feature flag (don't enable by default; we'll use it in M07).
- [ ] 13.2. Log the launch with `tracing::info!("editor-app v{} starting", env!("CARGO_PKG_VERSION"))`.
- [ ] 13.3. Commit: `feat(app): initialize tracing subscriber with env filter`.

### 14. Add the `crates/editor-render/src/gpu.rs` module properly

- [ ] 14.1. Move the inline `GpuContext` from the previous draft into its own module `crates/editor-render/src/gpu.rs`. Keep the public API minimal: `GpuContext::new(window: Arc<Window>) -> Result<Self, RenderError>`, `GpuContext::resize(&mut self, new_size: PhysicalSize<u32>)`, `GpuContext::render_clear(&mut self, color: wgpu::Color)`.
- [ ] 14.2. Define `RenderError` in `crates/editor-render/src/error.rs` using `thiserror`. Cover: no suitable adapter, surface creation failure, device request failure.
- [ ] 14.3. Write rustdoc for every public item.
- [ ] 14.4. Commit: `feat(render): extract GpuContext module with error types and docs`.

### 15. Verify cross-platform builds

- [ ] 15.1. Push everything to a branch (`m01-scaffolding` if you prefer, otherwise `main` with frequent commits). Watch CI.
- [ ] 15.2. If Windows CI fails due to missing build tools, add a setup step: `uses: ilammy/msvc-dev-cmd@v1` to ensure MSVC is available.
- [ ] 15.3. If Linux CI fails on the `wgpu` build due to missing system packages, extend the `apt-get install` list in the workflow.
- [ ] 15.4. If macOS CI fails due to Metal issues, investigate; this usually means the wgpu version on crates.io is incompatible with the macOS runner's Metal version — bump wgpu if so.
- [ ] 15.5. Iterate until all three OSes are green on both `clippy` and `test` jobs.
- [ ] 15.6. Commit: `ci(fix): ensure cross-platform CI green`.

### 16. Document everything new

- [ ] 16.1. Update `/docs/TECH_STACK.md` with the actually-pinned versions from step 2.
- [ ] 16.2. Update `/docs/ARCHITECTURE.md` with the actual crate layout (matches what you built).
- [ ] 16.3. Update `/DEVELOPMENT.md` with the Windows Build Tools install instructions, Linux apt-get list, macOS Xcode CLT.
- [ ] 16.4. Update `/docs/STATUS.md`: M01 complete, M02 next.
- [ ] 16.5. Update `/CHANGELOG.md` under `## [Unreleased]`:
  ```
  ### Added
  - Cargo workspace with six member crates.
  - Pinned Rust stable X.YY.Z toolchain.
  - GitHub Actions CI on Windows, Linux, and macOS.
  - cargo-deny and cargo-audit security gates.
  - Hello-window smoke test (winit + wgpu).
  ```
- [ ] 16.6. Commit: `docs: update after M01 scaffolding`.

### 17. Self-review

- [ ] 17.1. Run the full quality gate locally: `cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features && cargo build --release --workspace && cargo run --release --bin editor-app`.
- [ ] 17.2. Confirm CI is green on all three OSes for the final commit.
- [ ] 17.3. Browse the repo on GitHub. Check that README renders well, crate structure is clear, and CI badge (add one to README) shows green.
- [ ] 17.4. Tag: `git tag -a m01-complete -m "M01 complete: repo scaffolding and CI"`; push tag.

---

## Validation / Acceptance Criteria

M01 is complete when **all** of the following are true:

1. `cargo build --release --workspace` succeeds on Windows, Linux, and macOS.
2. `cargo test --workspace --all-features` succeeds on all three.
3. `cargo clippy --workspace --all-targets --all-features -- -D warnings` produces zero warnings on all three.
4. `cargo fmt --all -- --check` passes with no changes needed.
5. `cargo run --release --bin editor-app` opens a blank dark window on Windows, closes cleanly.
6. GitHub Actions CI is green on the `main` branch.
7. The workspace has exactly the six member crates from the architecture doc.
8. `rust-toolchain.toml` pins an exact stable version.
9. `deny.toml` and `cargo deny check` pass.
10. `docs/STATUS.md` reflects "M01 done, M02 next."
11. A `m01-complete` tag is pushed.

## Testing Requirements

- Unit tests: the stub `CoreError` enum should have at least one doctest.
- Integration test: `editor-app`'s `--dry-run` smoke test.
- CI: covers `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo build --release` on 3 OSes.
- Manual smoke: `cargo run --release --bin editor-app` opens a window on Windows.

## Git Commit Strategy

Expect roughly 12-16 commits. Push at least after items 3, 6, 7, 11, 15, and 16.

Use Conventional Commits: `feat`, `fix`, `docs`, `build`, `ci`, `test`, `refactor`, `chore`.

## Handoff to M02

M02 assumes:

- `editor-core` crate exists and compiles as an empty library with `CoreError`/`CoreResult`.
- `Cargo.toml` has `ropey`, `thiserror`, `tracing`, `criterion` (dev), `proptest` (dev) wired.
- CI runs on the new code automatically.
- `docs/TEXT_ENGINE.md` is the reference for how the rope-based text engine should behave.

M02 will flesh out `editor-core`: rope wrapper, cursor, selection, undo/redo, benches.

---

## Standing Orders Reminder

- Do not stop until every TODO is done.
- If any CI job is red at the end, M01 is not complete.
- Push often. Do not let 20 local commits pile up unpushed.
- If `wgpu` or `winit` has moved to a breaking version since this mission was written, adapt — do not pin to an outdated version just to match this doc. Update the doc instead.
- If you hit a Windows-specific issue (common with native windowing), document the fix in `/docs/CROSS_PLATFORM.md` and link from the code with a comment: `// See docs/CROSS_PLATFORM.md#windows-long-paths`.

---

## As-built verification (2026-04)

This repository **satisfies M01’s intent** on `main`: virtual workspace, pinned `rust-toolchain.toml`, `rustfmt.toml` / `.clippy.toml` / `deny.toml` / `.editorconfig`, GitHub Actions (`ci.yml`, `audit.yml`, `bench.yml`), headless `editor-app --dry-run` + integration test, Windows `app.manifest` (long paths + UTF-8 code page), `.githooks/pre-commit`, and `editor-render::GpuContext` in `gpu.rs`.

**Difference from the original spec:** the workspace has **eight** members, not six — `crates/editor-diff` and `crates/editor-workspace` were added in later missions. The original six remain; treat M01 acceptance criterion #7 as “those six exist, plus documented extensions.”

Go.
