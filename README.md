# IDE

A ground-up, native, high-performance code editor written in Rust.

**Status:** Pre-alpha. Mission M00 (Foundation Research & Documentation) complete.
See `docs/STATUS.md` for the up-to-date mission state.

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

See `docs/PERFORMANCE_MODEL.md` for the full budget breakdown.

## Repository Layout

```
/                         # workspace root (this file)
├── Cargo.toml            # virtual workspace manifest (added in M01)
├── ARCHITECTURE.md       # high-level system architecture
├── TECH_STACK.md         # technology choices and rationale
├── CHANGELOG.md          # release notes (Keep A Changelog format)
├── FOLLOWUPS.md          # deferred concerns (created on demand)
├── docs/                 # long-form reference documentation (see docs/README.md)
├── crates/               # workspace member crates (added in M01)
│   ├── editor-core/      # rope, cursor, undo/redo, document model
│   ├── editor-render/    # wgpu pipeline, glyph atlas, layout
│   ├── editor-input/     # OS input translation and command dispatch
│   ├── editor-io/        # file load/save, mmap, atomic writes
│   └── editor-app/       # binary crate that wires subsystems into a shell
└── .github/workflows/    # CI matrix: Windows, Linux, macOS
```

## Quickstart (After M01)

```powershell
# Build and run in release mode
cargo run --release

# Run the full quality gate
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
```

On Linux and macOS, substitute your shell of choice; the same cargo commands apply.

## Mission-Driven Development

Development proceeds through a fixed sequence of 12 missions (M00–M11). Each
mission leaves the repository buildable, testable, and runnable on all three
target platforms. See `docs/MISSIONS.md` for the index and `docs/STATUS.md`
for the current mission state.

## License

Dual-licensed under either of:

- Apache License, Version 2.0 (`LICENSE-APACHE` or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license (`LICENSE-MIT` or <http://opensource.org/licenses/MIT>)

at your option.

## Contributing

This is a personal/experimental project currently following a strict
mission-driven agent workflow. See `docs/MISSIONS.md` and the mission standing
orders in `docs/AGENT_GUIDELINES.md` for how changes are introduced.
