# `editor-app`

Binary shell. Owns the `winit` event loop (`ApplicationHandler`), wires
subsystems together, and initializes `tracing`.

Produces the `editor-app` binary (`cargo run --release --bin editor-app`).
`--dry-run` skips the window and runs headless `wgpu` adapter/device init.

See `docs/ARCHITECTURE.md` for wiring.
