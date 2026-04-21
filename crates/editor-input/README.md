# `editor-input`

OS input event translation. No edits applied here; we return high-level
actions to `editor-app`. Real-time path.

See `docs/INPUT_AND_IME.md`. Scaffolded in M01; translator implemented in
M05.

- **Testing:** use [`map_keyboard_input`](src/lib.rs) (physical key + optional text + state) in unit tests; `KeyEvent` is not constructible off-crate in winit 0.30+.
- **Bench:** `cargo bench -p editor-input --bench translate` (baseline name `m05-mvp` in `docs/PERFORMANCE_BUDGETS.md`).
