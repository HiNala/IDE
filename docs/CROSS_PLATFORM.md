[← docs/](./) · [README](../README.md)

# Cross-platform windowing and input

Notes for **winit 0.30** + **wgpu** on **Windows**, **Linux**, and **macOS**.
Updated for **M12** (resize, DPI, multi-monitor).

## Coordinate spaces

- **Logical** sizes and positions are DPI-independent; **physical** pixels scale by `scale_factor()`.
- All layout and hit-testing in the editor shell use **physical** pixel units for the client area unless noted (matches `FrameInput::physical_size`).

## Resize

- Every **inner** size change produces `WindowEvent::Resized`. The app **reconfigures** the `wgpu` surface and **paints in the same handler** so the client area updates during continuous drag (critical on **Windows**, where the event loop can behave modally during edge-drag).
- **Zero-sized** surfaces are skipped (minimize or transient states): we do not `configure` with `0×0`.
- **Text:** visible-line cosmic-text buffers are pre-sized to a fixed row cap so tall viewports do not realloc scratch `Vec`s on every height change (see `RENDERING_PIPELINE.md`).

## DPI and scale factor

- `WindowEvent::ScaleFactorChanged` updates `FrameInput::scale_factor` and invalidates glyph layout/rasterization as needed (see `TextLayer::set_scale_factor`).
- Dragging between monitors with different scale factors produces a scale change when winit reassigns the window to the new monitor.

## Multi-monitor

- **Present mode** and **refresh** hints follow the **current** monitor (`Window::current_monitor()`), updated on `Moved`, `Resized`, and `ScaleFactorChanged`.
- **Persisted state** (`state.json`) may record **per-monitor** scale factors keyed by `MonitorHandle::name()` (when the OS provides a name) for debugging and future restoration hints.
- Unplugging a monitor: expect `Resized` / `Moved` from the OS; the shell clamps scroll and reconfigures the surface.

## Fullscreen and shortcuts

- **F11** toggles **borderless fullscreen** (`Fullscreen::Borderless`); **Alt+Enter** does the same.
- **Ctrl+F11** (primary modifier + F11: **Cmd+F11** on macOS) toggles the **dev metrics HUD** so it does not collide with fullscreen.

## Platform-specific pitfalls

| Topic | Windows | Linux (X11/Wayland) | macOS |
|--------|---------|---------------------|--------|
| Modal resize loop | Paint on each `Resized` | Less aggressive | Less aggressive |
| Monitor name | Often present | Varies by compositor | Present |
| Vulkan / GL | DX12 / Vulkan / GL | Vulkan / GL | Metal only (via wgpu) |

**Environment:** `WGPU_BACKEND` overrides the default backend bitmask from `editor-render::backend`.
`IDE_PRESENT_MODE` forces a specific `PresentMode` when the surface supports it.
`IDE_POWER_PREFERENCE=low` prefers integrated/efficiency adapters before discrete.
Ubuntu CI `--dry-run` often selects **llvmpipe** (CPU); that is expected.

## File I/O (M06)

- **Atomic save:** `editor-io` writes to a `tempfile::NamedTempFile` in the target directory, `sync_all`s, then `persist` (rename over the destination). On **NTFS** and typical **ext4**/**APFS** volumes this is the standard crash-safe pattern. On **NFS** or exotic network filesystems, rename atomicity may be weaker; we document the caveat and still prefer temp-then-rename over in-place overwrite.
- **Windows long paths:** The repo’s manifest / toolchain opts into long paths where supported; `PathBuf` round-trips Unicode and extended-length paths.
- **Reserved names:** Final path components matching `CON`, `NUL`, `COM1`, … are rejected before write (see `editor_io::paths::is_windows_reserved_path`).
- **Large reads:** Files ≥ `editor_io::MMAP_THRESHOLD_BYTES` use `memmap2` with a streaming read fallback; long streaming reads emit `LoadProgress` updates (see `editor-io`).

## See also

- [`RENDERING_PIPELINE.md`](RENDERING_PIPELINE.md) — GPU stack and present policy.
- [`DIAGNOSING_PERFORMANCE.md`](DIAGNOSING_PERFORMANCE.md) — resize telemetry (`--resize-telemetry`) and artifact catalog; CI **`m12-gpu-resize-windows`** runs `gpu_resize_stress` on Windows.
- [`reference/03_GAPS_AND_RISKS.md`](../reference/03_GAPS_AND_RISKS.md) — PRD risks (§ DPI / cross-platform).
