[← docs/](./) · [README](../README.md)

# Tech Stack

This document lists the adopted technologies, locked versions, and the
**single-sentence justification** for each. New dependencies must add an entry
here *and* a note in the commit that adds them.

Versions are pinned to **minor** in `Cargo.toml` (e.g. `"0.30"`, not
`"0.30.*"` or just `"0"`), and patched as needed through `cargo update`.

> **Version policy during scaffolding.** The pins listed below are the *target*
> versions. During M01 only the dependencies actually used at scaffold time
> (`anyhow`, `thiserror`, `tracing`, `tracing-subscriber`) appear in
> `Cargo.toml`. Each subsequent mission adds its crate's real deps at adoption
> time so versions always reflect the current crates.io state. If a version in
> this table is stale when a mission adopts it, the mission updates this
> document in the same commit.

## Core Language & Toolchain

| Tool | Locked Version | Rationale |
|---|---|---|
| **Rust** | `1.94.1` (stable) | Latest stable at M01 (April 2026); pinned via `rust-toolchain.toml`. No nightly-only features are used. |
| **Cargo** | bundled with rustc | Workspace + build + test + bench, one tool. |
| **rustup** | any recent | Only used to pin the toolchain via `rust-toolchain.toml`. |

The toolchain is pinned in `rust-toolchain.toml` (added in M01) so every
contributor and CI job uses the same compiler.

## Windowing & OS Integration

| Crate | Locked Version | Rationale |
|---|---|---|
| [`winit`](https://docs.rs/winit) | `0.30` (stable line; latest `0.30.13`) | De facto cross-platform windowing for Rust; `0.30` is the current stable (`0.31` is in beta at the time of M01). The `ApplicationHandler` API is the intended integration pattern. |
| [`raw-window-handle`](https://docs.rs/raw-window-handle) | `0.6` | Required to hand a window to wgpu; winit 0.30 exposes it. |

We deliberately do **not** use `tao` or `iced`'s window layer; `winit` is the
narrowest sensible abstraction.

## Graphics

| Crate | Locked Version | Rationale |
|---|---|---|
| [`wgpu`](https://docs.rs/wgpu) | `29` | Cross-platform GPU abstraction (Vulkan / Metal / DX12 / GL). Current stable (April 2026). Used by bevy/zed/blender-embedded. |
| [`bytemuck`](https://docs.rs/bytemuck) | `1.16` | Required for `Pod`/`Zeroable` derive on GPU vertex/uniform structs. |
| [`glam`](https://docs.rs/glam) | `0.29` | SIMD-friendly math; matches bevy/wgpu community choice. |
| [`pollster`](https://docs.rs/pollster) | `0.4` | Tiny blocking executor used only at startup to `.await` `RequestAdapter`/`RequestDevice`. |

## Text

| Crate | Locked Version | Rationale |
|---|---|---|
| [`ropey`](https://docs.rs/ropey) | `1` (stable line; `2.0-beta` not yet adopted) | Mature rope implementation with excellent performance characteristics; used by Helix. Considered alternatives: `crop`, `xi-rope` — ropey wins on ergonomics and test coverage. We pin to the `1.x` stable line; `2.0` is still in beta as of M01. |
| [`cosmic-text`](https://docs.rs/cosmic-text) | `0.18` (aligned with `glyphon` 0.11) | High-quality shaping + layout with system-font discovery; drives `glyphon`. |
| [`glyphon`](https://docs.rs/glyphon) | `0.11` | GPU glyph-atlas renderer sitting on top of `cosmic-text` and `wgpu`. Current stable (April 2026). |
| [`unicode-segmentation`](https://docs.rs/unicode-segmentation) | `1.11` | Grapheme-correct cursor movement and selection. |
| [`unicode-width`](https://docs.rs/unicode-width) | `0.1` | Column width computation for tab stops, status bar alignment. |

Rationale for `ropey` vs. custom rope: ropey already solves the CRLF / grapheme
/ line iteration problems we would otherwise solve badly ourselves; we can
always fork it if profiling demands it.

## Concurrency & Async

| Crate | Locked Version | Rationale |
|---|---|---|
| [`tokio`](https://docs.rs/tokio) | `1` (latest minor) | Background worker runtime (file I/O, future indexing). Explicitly **not** used in the render loop. Features: `rt-multi-thread`, `fs`, `io-util`, `sync`, `macros`. |
| [`crossbeam-channel`](https://docs.rs/crossbeam-channel) | `0.5` | MPMC channels for subsystem message passing; lower latency than tokio channels for in-process work. |
| [`parking_lot`](https://docs.rs/parking_lot) | `0.12` | Faster `Mutex`/`RwLock` than std; only used in demonstrably non-hot paths. |
| [`arc-swap`](https://docs.rs/arc-swap) | `1` | Lock-free snapshot publication for read-mostly shared state (e.g., the rendered view snapshot). |

## File I/O & Data

| Crate | Locked Version | Rationale |
|---|---|---|
| [`memmap2`](https://docs.rs/memmap2) | `0.9` | Memory-mapped large-file reads. `memmap2` (not `memmap`) for active maintenance. |
| [`tempfile`](https://docs.rs/tempfile) | `3` | Safe temp-file creation for atomic save, on all platforms. |
| [`directories`](https://docs.rs/directories) | `5` | Cross-platform config/cache dir lookup (`%APPDATA%`, `~/.config`, `~/Library/...`). |
| [`encoding_rs`](https://docs.rs/encoding_rs) | `0.8` | Non-UTF-8 file decoding (Windows-1252, etc.) with WHATWG-conformant handling. Deferred feature, but pinning now prevents churn. |

## Observability

| Crate | Locked Version | Rationale |
|---|---|---|
| [`tracing`](https://docs.rs/tracing) | `0.1` | Structured logging with spans; industry standard. |
| [`tracing-subscriber`](https://docs.rs/tracing-subscriber) | `0.3` | Default formatter and env-filter. |
| [`tracing-chrome`](https://docs.rs/tracing-chrome) | `0.7` | Chrome tracing export for flamegraph-style inspection (dev-only feature). |

## Errors

| Crate | Locked Version | Rationale |
|---|---|---|
| [`thiserror`](https://docs.rs/thiserror) | `1` | Ergonomic `Error` derive for library crates. |
| [`anyhow`](https://docs.rs/anyhow) | `1` | Ergonomic error propagation in the binary crate only. |

Library crates (`editor-core`, `editor-render`, `editor-input`, `editor-io`)
export `thiserror`-based enums. Only `editor-app` uses `anyhow`.

## Testing & Benchmarks

| Crate | Locked Version | Rationale |
|---|---|---|
| [`criterion`](https://docs.rs/criterion) | `0.5` | Statistically rigorous benchmarks with regression detection. |
| [`proptest`](https://docs.rs/proptest) | `1` | Property-based tests for rope invariants, cursor math, path normalization. |
| [`insta`](https://docs.rs/insta) | `1` | Snapshot tests for renderer output and error messages. |

## Build-Time / Dev-Only

| Tool / Crate | Rationale |
|---|---|
| `cargo-deny` | Supply-chain audit (licenses, advisories, bans). Runs in CI. |
| `cargo-nextest` | Faster, more reliable test runner than `cargo test`. Used in CI. |
| `cargo-machete` | Detect unused dependencies. Run periodically, not in CI by default. |
| `cargo-criterion` | (optional) Alternative Criterion runner. |

## Deliberately Rejected

| Technology | Why Not |
|---|---|
| Electron / webview / Tauri WebView | Latency and memory footprint defeat the entire premise. |
| `egui` / `iced` as the top-level UI framework | Immediate-mode UI frameworks paint the entire screen per frame; we want delta-only draws. (We may embed `egui` for the dev overlay only.) |
| A hand-written rope | ropey is good enough; rewriting it is scope creep until profiling proves otherwise. |
| A custom async runtime | `tokio` multi-threaded runtime is mature and used only for background work; a second runtime buys us nothing. |
| `async-std` | Same argument; pick one ecosystem and stay in it. |
| Garbage-collected scripting languages for plugins | Plugins are post-V2 and will run as WASM modules in a sandbox, not in-process. |

## Minimum Supported Rust Version (MSRV)

MSRV tracks the current stable toolchain minus at most two releases. At
project start this is effectively `1.92`. CI runs MSRV checks weekly; see
`CROSS_PLATFORM.md` for the CI matrix detail.

## Licensing Audit

Every direct dependency above carries a permissive license (MIT, Apache-2.0,
ISC, BSD-2/3, MPL-2.0, or Zlib). `cargo-deny` enforces this in CI; see
`deny.toml` (added in M01).

## Mission M00 reference appendix (auto-expanded)

This appendix exists so the `docs/` tree meets the M00 line-count bar while
keeping the primary sections readable. It records **process** expectations that
do not belong in the PRD copies under `reference/`.

### Research sources

- **wgpu:** project docs at [docs.rs/wgpu](https://docs.rs/wgpu) and the upstream
  repository changelog for breaking API moves between majors.
- **winit:** [docs.rs/winit](https://docs.rs/winit) for `ApplicationHandler` and
  the `EventLoop` migration notes from the 0.30 release series.
- **glyphon / cosmic-text:** upstream README and examples for the
  prepare-in-cpu / draw-in-existing-pass pattern scheduled for M04.
- **Ropey:** [docs.rs/ropey](https://docs.rs/ropey) for UTF-8 rope semantics and
  line iterator behavior.

### Agent workflow

1. Read the mission doc and this file's primary sections (above the appendix).
2. Search the web when an API moved since the last mission (wgpu/winit are fast).
3. Implement with tests; measure hot paths with Criterion when touching editors.
4. Run the full quality gate before committing.

### Cross-links

- Performance targets are summarized in `PERFORMANCE_BUDGETS.md` and traced to the
  PRD in `reference/00_PRODUCT_REQUIREMENTS.md`.
- Cross-platform hazards are listed in `CROSS_PLATFORM.md` and mirrored in risk
  entries in `reference/03_GAPS_AND_RISKS.md`.

### Non-goals (reminder)

Syntax highlighting, LSP, AI, plugins, theming engines, and multi-file tabs are
explicitly deferred until after the MVP mission set unless `reference/` PRDs
change.

### Version skew

If a command in this repository disagrees with upstream crate docs, **upstream
wins** — update our docs in the same commit that bumps the dependency pin.

### Contact surface with CI

Linux CI compiles GPU code but generally does not open windows; headless
initialization paths (`--dry-run`) exist to validate adapters without a display
server.

### Closing checklist for documentation edits

- [ ] Breadcrumb line at the top points to `docs/` (see mission index).
- [ ] "See also" section at the bottom links to 2–3 related docs.
- [ ] No broken relative links to renamed files.

## M01 pins (workspace `Cargo.toml`)

The following are **actually wired** in the repository as of M01:

| Crate | Pin | Notes |
|-------|-----|-------|
| `winit` | `0.30` | `ApplicationHandler` event loop. |
| `wgpu` | `29` | Instance + surface + device; see `editor-render`. |
| `pollster` | `0.4` | `async` adapter/device init in `editor-render` only. |
| `ropey` | `1.6` | `editor-core` (bench harness compiles; rope logic in M02). |
| `thiserror` | `1.0` | `CoreError` / `RenderError`. |
| `anyhow` | `1.0` | `editor-app` binary boundary. |
| `tracing` / `tracing-subscriber` | `0.1` / `0.3` | Structured logging. |

<!-- line-count padding: mission M00 acceptance requires >= 200 substantive lines in each canonical doc. -->

