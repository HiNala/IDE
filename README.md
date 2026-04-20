# IDE

A ground-up, native, high-performance code editor written in Rust.

[![CI](https://github.com/HiNala/IDE/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/HiNala/IDE/actions/workflows/ci.yml)

**Status:** Pre-alpha — see [`docs/STATUS.md`](docs/STATUS.md).

---

## Quick start

```bash
git clone https://github.com/HiNala/IDE.git
cd IDE
cargo run --release --bin editor-app
```

Expect a dark window; close it to exit. Headless GPU smoke (CI-friendly):

```bash
cargo run --release --bin editor-app -- --dry-run
```

Prerequisites: [DEVELOPMENT.md](DEVELOPMENT.md).

---

## What This Is

This project builds a native code editor that is measurably, provably, and
consistently faster than Electron-based tools like VS Code and Cursor. It treats
the editor as a **real-time engine for structured text manipulation** rather
than a productivity application, borrowing its architectural model from game
engines and databases.

Key architectural commitments:

- **Rust** (stable) for the entire codebase — no GC pauses, no runtime, no VM.
- **`winit`** for cross-platform windowing with a minimal abstraction surface.
- **`wgpu`** for GPU-accelerated rendering on Vulkan, Metal, DX12, and GL fallback.
- **Rope** data structure for the text buffer, with arena-backed chunks.
- **Frame-based execution loop** with explicit per-frame performance budgets.
- **Ownership-based concurrency** with isolated subsystems and no shared mutable state
  in the real-time path.

## Performance Targets (Hard)

| Metric | Target |
|---|---|
| Input-to-pixel latency | < 5 ms under normal load |
| Frame rate during scroll/edit | 60 fps minimum, 120 fps where hardware allows |
| Cold start | < 1 s on modern hardware |
| 100 MB file open | Non-blocking; UI stays responsive |
| Memory growth | Bounded; stable over multi-hour sessions |

See `docs/PERFORMANCE_BUDGETS.md` for the full budget breakdown.

## Repository Layout

```
/                         # workspace root (this file)
├── Cargo.toml            # virtual workspace manifest
├── CHANGELOG.md          # release notes (Keep A Changelog format)
├── CONTRIBUTING.md       # contributor contract (read first)
├── DEVELOPMENT.md        # how to build and run locally, per OS
├── FOLLOWUPS.md          # deferred concerns (created on demand)
├── docs/                 # reference documentation (see docs/README.md)
│   ├── ARCHITECTURE.md   # high-level system architecture
│   ├── TECH_STACK.md     # technology choices and rationale
│   └── ...               # subsystem docs, PRDs, status, missions
├── reference/            # verbatim source PRDs (do not edit)
├── crates/               # workspace member crates
│   ├── editor-core/      # rope, cursor, undo/redo, document model
│   ├── editor-render/    # wgpu pipeline, glyph atlas, layout
│   ├── editor-input/     # OS input translation and command dispatch
│   ├── editor-io/        # file load/save, mmap, atomic writes
│   ├── editor-ui/        # gutter, status bar (V2), minimal chrome
│   └── editor-app/       # binary crate that wires subsystems into a shell
└── .github/workflows/    # CI matrix: Windows, Linux, macOS
```

## Quality gate

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo build --release --workspace --locked
```

On Linux and macOS, substitute your shell of choice; the same cargo commands apply.

## Mission-Driven Development

Development proceeds through a fixed sequence of 12 missions (M00–M11). Each
mission leaves the repository buildable, testable, and runnable on all three
target platforms. See `docs/MISSIONS.md` for the index and `docs/STATUS.md`
for the current mission state.

The root documents to read first:

- `docs/ARCHITECTURE.md` — the system shape in one page.
- `docs/TECH_STACK.md` — what we're building it with.
- `CONTRIBUTING.md` — ground rules for any contributor (human or agent).
- `DEVELOPMENT.md` — local build setup per OS.
- `reference/` — verbatim product requirements (source of truth).

## License

Dual-licensed under either of:

- Apache License, Version 2.0 (`LICENSE-APACHE` or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license (`LICENSE-MIT` or <http://opensource.org/licenses/MIT>)

at your option.

## Contributing

This is a personal/experimental project currently following a strict
mission-driven agent workflow. See `CONTRIBUTING.md` for the contributor
contract and `docs/AGENT_GUIDELINES.md` for the agent-specific standing
orders.
