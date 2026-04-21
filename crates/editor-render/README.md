# `editor-render`

GPU rendering via `wgpu`. Owns all GPU resources. Real-time path.

See `docs/RENDERING_PIPELINE.md`. `GpuContext` + `EditorRenderer`, `backend` / `timing` modules,
`--dry-run` via `dry_run_headless`, Criterion benches `frame_overhead` and `text_layer_prepare`
(M04; baseline `m04-mvp`), ignored GPU test `tests/visual_smoke.rs`.
