# `editor-app`

Binary shell. Owns the `winit` event loop, wires subsystems together, and
initializes `tracing`.

Produces the `ide` binary (`[[bin]] name = "ide"`).

See `ARCHITECTURE.md` (root) for the wiring. Scaffolded in M01 as a boot
banner; M03 grows it into a real windowed app.
