# M03 — Windowing & wgpu Rendering Foundation

**Mission ID:** M03
**Prerequisites:** M02 complete. `editor-core` is usable. The smoke-test window from M01 exists.
**Output:** A robust `editor-render` crate that owns the full wgpu pipeline: instance, adapter, device, queue, surface, swapchain, render pass. Handles resize, DPI changes, backend selection, and graceful degradation. The `editor-app` binary boots into a window with the correct backend on every OS, renders a parametric clear color (set via a test command) every frame, and measures its own frame time.
**Estimated scope:** 2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/docs/RENDERING_PIPELINE.md` — our frame-based rendering approach.
- `/docs/ARCHITECTURE.md` — `editor-render`'s responsibilities and what it must not touch.
- `/docs/CROSS_PLATFORM.md` — DX12/Metal/Vulkan backend considerations.
- `/docs/INPUT_AND_IME.md` — winit's `ApplicationHandler` model (we'll wire input thoroughly in M05, but the window lifecycle matters here).
- `/docs/PERFORMANCE_BUDGETS.md` — frame-timing targets.
- `https://docs.rs/wgpu/latest/wgpu/` — current wgpu API. **Verify the pinned version matches what's current on crates.io before you start.** wgpu moves fast.
- `https://docs.rs/winit/latest/winit/` — especially the `ApplicationHandler` trait.
- `https://sotrh.github.io/learn-wgpu/` — the learn-wgpu tutorial; chapter 2 on surface + swapchain is directly relevant.

---

## The Situation In Plain English

M01 gave us a "Hello, window" smoke test with a minimal `GpuContext`. That was enough to prove wgpu and winit work together. M03 takes that sketch and turns it into the real rendering foundation: a crate that owns every GPU resource lifecycle the editor needs, handles every platform's quirks, and exposes a clean API that the text renderer (M04) can build on top without ever touching `wgpu` directly from outside `editor-render`.

The hard part is not writing a wgpu tutorial. The hard parts are:

1. **Backend selection**: DirectX 12 on Windows 10+ (with DX11 fallback for older hardware, mirroring Zed's October 2025 Windows launch strategy), Metal on macOS, Vulkan on Linux (with GL fallback). The `wgpu::Backends` bitmask and fallback adapter logic need to be airtight.
2. **Resize and DPI**: when the window is resized, or the user drags it between a 1x and 2x monitor, the swapchain needs to reconfigure without flickering, hanging, or leaking resources. This is especially painful on Windows with fractional scaling.
3. **VSync and frame pacing**: `PresentMode::FifoRelaxed` is the default for VSync-locked low-latency. `PresentMode::Immediate` is available behind an env var for benchmarking.
4. **Surface lost / outdated**: on some platforms (especially Linux/Vulkan), the surface can become lost or outdated during resize or sleep/wake. We must detect and recover, not crash.
5. **Frame timing**: we record `Instant::now()` at the start of each frame and compute the frame delta. This number is later surfaced via `tracing` spans and the dev overlay (M07). For M03, just capture the data cleanly.
6. **Multithreading readiness**: the architecture docs say rendering and state mutation run on different threads (main thread owns input + coordination, render thread does GPU submission). We will not yet split threads in M03 — that comes in M05 — but we design the API so adding the split in M05 is a local change, not a rewrite.

All of this happens inside `editor-render`. The `editor-app` binary uses the `editor-render` API and shrinks accordingly (most of what's in `main.rs` today moves into `editor-render`).

---

## Scope

**In scope:**
- `editor-render::Renderer` — the top-level struct that owns GPU resources.
- Backend selection with fallbacks.
- Robust resize and DPI handling.
- Configurable present mode (VSync-locked default, Immediate via env var).
- Surface-lost / surface-outdated recovery.
- Per-frame timing capture.
- A minimal `RenderFrame` abstraction that represents one frame's worth of work (acquire surface texture → begin render pass → record draws → end pass → submit → present).
- A `ClearColor` placeholder draw so the window renders something visible. Text comes in M04.
- Headless / dry-run initialization path for CI.

**Out of scope:**
- Text rendering (M04).
- Input handling (M05; here we only handle window lifecycle events: Resized, CloseRequested, RedrawRequested, ScaleFactorChanged).
- UI elements (M09).
- Multithreaded render loop (M05).

---

## North Star

At the end of M03, `cargo run --release --bin editor-app` opens a window that:

- Shows an animated clear color (e.g., a slow rainbow cycle) so you can visually confirm the render loop is alive.
- Resizes smoothly: drag an edge, the rectangle reflows without stutter or crash.
- Handles DPI changes: drag the window between a 1x and 2x monitor; the clear color keeps rendering correctly, the swapchain reconfigures.
- Reports frame times via `tracing::debug!` (visible with `RUST_LOG=editor_render=debug`).
- Can run in `--dry-run` mode (CI path): initializes wgpu in headless mode, confirms a compatible adapter exists, exits 0.

And on every supported OS:

- Windows: DX12 by default, DX11 or Vulkan as fallback.
- macOS: Metal.
- Linux: Vulkan by default, GL as fallback.

---

## TODO List

### 1. Refactor `editor-render` into a proper module layout

- [ ] 1.1. Current layout (from M01) has `GpuContext` in a single file. Break it out:
  ```
  crates/editor-render/src/
  ├── lib.rs              (public re-exports)
  ├── error.rs            (RenderError)
  ├── gpu.rs              (GpuHandle: instance, adapter, device, queue — the hardware abstraction)
  ├── surface.rs          (SurfaceManager: owns the wgpu::Surface, handles resize and reconfigure)
  ├── renderer.rs         (Renderer: high-level per-frame API, owns GpuHandle + SurfaceManager)
  ├── frame.rs            (RenderFrame: a scope-guarded frame object)
  ├── backend.rs          (Backend selection logic: chooses DX12/Metal/Vulkan and fallbacks)
  └── timing.rs           (FrameTimer: captures frame deltas, exposes last-N frame statistics)
  ```
- [ ] 1.2. Move the existing smoke-test logic from M01 into the new structure. Keep the tests working throughout.
- [ ] 1.3. Commit: `refactor(render): split editor-render into modules (gpu, surface, renderer, frame, backend, timing)`.

### 2. Implement backend selection

- [ ] 2.1. In `backend.rs`, implement a `Backend` enum with variants for each backend and a `select_backends(requested: Option<&str>) -> wgpu::Backends` function.
- [ ] 2.2. Default: Windows → `wgpu::Backends::DX12 | wgpu::Backends::VULKAN` (DX12 first); macOS → `wgpu::Backends::METAL`; Linux → `wgpu::Backends::VULKAN | wgpu::Backends::GL`.
- [ ] 2.3. Respect the `WGPU_BACKEND` environment variable (wgpu already does this internally via `backend_bits_from_env`; confirm and wire through).
- [ ] 2.4. When requesting the adapter, use `RequestAdapterOptions` with `force_fallback_adapter: false` first; if that fails, retry with `force_fallback_adapter: true` and `power_preference: LowPower`. This gives us a CPU-fallback software rendering path for systems without a real GPU (including headless CI on Linux).
- [ ] 2.5. Log the selected backend and adapter info at `info!`: name, vendor, device type (discrete/integrated/virtual/CPU), driver version.
- [ ] 2.6. Test: on Windows, log shows `Backend: Dx12` (or `Vulkan` if DX12 is not available).
- [ ] 2.7. Commit: `feat(render): implement backend selection with fallback`.

### 3. Implement `GpuHandle`

- [ ] 3.1. `GpuHandle` bundles the four "hardware handles" we care about: `Instance`, `Adapter`, `Device`, `Queue`. These are the things that do not change over the editor's lifetime (unlike the surface, which changes on resize).
- [ ] 3.2. Async constructor (wgpu requires `async` to request adapter and device): `pub async fn new(backends: wgpu::Backends) -> Result<Self, RenderError>`.
- [ ] 3.3. The `editor-app` binary uses `pollster::block_on(GpuHandle::new(..))`; that's fine in `main` — do not add `tokio`.
- [ ] 3.4. Expose read-only accessors: `instance(&self) -> &wgpu::Instance`, `adapter(&self) -> &wgpu::Adapter`, `device(&self) -> &wgpu::Device`, `queue(&self) -> &wgpu::Queue`.
- [ ] 3.5. Keep `GpuHandle: Send + Sync` — required for future multi-threading in M05. This means the fields must themselves be `Send + Sync`; wgpu types already are. Add a compile-time assertion: `static_assertions::assert_impl_all!(GpuHandle: Send, Sync);` (add `static_assertions = "1"` to `Cargo.toml`).
- [ ] 3.6. Commit: `feat(render): implement GpuHandle with async constructor and Send+Sync assertion`.

### 4. Implement `SurfaceManager`

- [ ] 4.1. Owns the `wgpu::Surface<'static>` (use `'static` lifetime with `Arc<Window>`). Creation: `SurfaceManager::new(gpu: &GpuHandle, window: Arc<Window>) -> Result<Self, RenderError>`.
- [ ] 4.2. Owns the current `wgpu::SurfaceConfiguration`. Picks the surface format automatically: prefer `Bgra8UnormSrgb` or `Rgba8UnormSrgb` (whichever the adapter supports as the first choice).
- [ ] 4.3. Picks the present mode: default `FifoRelaxed` (or `Fifo` if `FifoRelaxed` is unsupported; check `SurfaceCapabilities::present_modes`). Respect env var `IDE_PRESENT_MODE=immediate|fifo|fifo_relaxed|mailbox`.
- [ ] 4.4. `reconfigure(&mut self, size: PhysicalSize<u32>)` — called on `WindowEvent::Resized`. Clamps to minimum 1×1 (never configure with 0-sized surface — wgpu panics or produces an error).
- [ ] 4.5. `acquire(&self) -> Result<wgpu::SurfaceTexture, SurfaceAcquireError>` — wraps `get_current_texture` with appropriate error handling for `SurfaceError::Lost`, `SurfaceError::Outdated`, `SurfaceError::Timeout`. On `Lost` or `Outdated`, the caller should reconfigure and retry.
- [ ] 4.6. `present(&self, tex: wgpu::SurfaceTexture)` — calls `tex.present()`.
- [ ] 4.7. Unit test: construct with a dummy surface (feature-gated behind `#[cfg(test)]` + headless wgpu instance); confirm reconfigure with 800×600 then 1920×1080 succeeds.
- [ ] 4.8. Commit: `feat(render): implement SurfaceManager with format negotiation and resize`.

### 5. Implement `FrameTimer`

- [ ] 5.1. Captures the timestamp at the start of each frame (`Instant::now()`) and computes the delta from the previous frame.
- [ ] 5.2. Stores a rolling window of the last 120 frame deltas (one second at 120 fps). Exposes:
  - `last_delta(&self) -> Duration`
  - `average_fps(&self) -> f32`
  - `p95_frame_time(&self) -> Duration`
  - `p99_frame_time(&self) -> Duration`
- [ ] 5.3. Tested with synthetic `Instant`s (use `std::time::Instant` — don't fake it, just test with real deltas by sleeping briefly).
- [ ] 5.4. Commit: `feat(render): add FrameTimer with rolling window statistics`.

### 6. Implement `Renderer`

- [ ] 6.1. The public top-level struct. Owns `GpuHandle`, `SurfaceManager`, `FrameTimer`. Publishes a minimal API surface:
  ```rust
  pub struct Renderer { /* ... */ }

  impl Renderer {
      pub async fn new(window: Arc<Window>) -> Result<Self, RenderError>;
      pub fn resize(&mut self, new_size: PhysicalSize<u32>);
      pub fn on_scale_factor_change(&mut self, new_scale: f64, new_size: PhysicalSize<u32>);
      pub fn render_frame(&mut self, clear: wgpu::Color) -> Result<(), RenderError>;
      pub fn timer(&self) -> &FrameTimer;
      pub fn gpu(&self) -> &GpuHandle;     // for M04 to attach glyphon
      pub fn surface_format(&self) -> wgpu::TextureFormat;  // M04 needs this
      pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration;  // M04 needs this
  }
  ```
- [ ] 6.2. `render_frame` flow:
  1. Start FrameTimer tick.
  2. Acquire surface texture. On `Outdated`/`Lost`, reconfigure and retry once. On second failure, return error.
  3. Create `CommandEncoder`.
  4. Begin render pass with the clear color as the `LoadOp::Clear` value.
  5. (No draws in M03. M04 will add text.)
  6. End render pass. Submit command buffer. Call `surface_texture.present()`.
  7. End FrameTimer tick and log if delta > 16ms (warn about frame budget violation).
- [ ] 6.3. `on_scale_factor_change` triggers a reconfigure with the new physical size. Log the event at `info!`.
- [ ] 6.4. `tracing::instrument` decorations on every method so spans show up in Tracy (when enabled in M07).
- [ ] 6.5. Commit: `feat(render): implement Renderer with frame submission and resize handling`.

### 7. Make `editor-app`'s `ApplicationHandler` robust

- [ ] 7.1. `main.rs` now holds a much smaller struct:
  ```rust
  struct App {
      window: Option<std::sync::Arc<winit::window::Window>>,
      renderer: Option<editor_render::Renderer>,
      /// For the visual liveness test: animates the clear color.
      frame_count: u64,
  }
  ```
- [ ] 7.2. Implement the full winit 0.30 `ApplicationHandler`:
  - `resumed(&mut self, event_loop: &ActiveEventLoop)`: create window with reasonable defaults (1280×720, "IDE" title, resizable, min inner size 400×300), then create `Renderer` via `pollster::block_on`. Store both. Request an immediate redraw.
  - `window_event`:
    - `WindowEvent::CloseRequested` → `event_loop.exit()`.
    - `WindowEvent::Resized(size)` → `renderer.resize(size)`, request redraw.
    - `WindowEvent::ScaleFactorChanged { scale_factor, .. }` → `renderer.on_scale_factor_change(scale_factor, window.inner_size())`.
    - `WindowEvent::RedrawRequested` → compute an animated clear color (e.g., `wgpu::Color { r: (frame_count as f64 * 0.01).sin().abs(), ... }`), call `renderer.render_frame(color)`, request redraw again (request-redraw-in-redraw is the idiomatic winit 0.30 way to keep the loop ticking).
  - `about_to_wait`: noop for now; don't request redraw here — it creates a tight loop. (We will revisit this in M05 with proper frame pacing.)
- [ ] 7.3. Wrap the whole thing so errors from `Renderer::new` or `Renderer::render_frame` log at `error!` level and exit cleanly, not panic.
- [ ] 7.4. Commit: `feat(app): integrate full winit ApplicationHandler with Renderer`.

### 8. Handle the `--dry-run` CI path

- [ ] 8.1. Detect `--dry-run` in `main.rs` before creating the event loop.
- [ ] 8.2. On dry-run: initialize `GpuHandle` only (no window, no surface). Log adapter info. Exit 0.
- [ ] 8.3. Test in CI: all three OSes' `cargo run -- --dry-run` exits 0. Document the fact that the Ubuntu CI runner uses the `llvmpipe` / CPU adapter for this.
- [ ] 8.4. Commit: `feat(app): --dry-run path for headless CI verification`.

### 9. Handle edge cases

- [ ] 9.1. Window minimized (`size.width == 0 || size.height == 0`): skip reconfigure and skip `render_frame`. Resume on next nonzero resize.
- [ ] 9.2. Window moved between displays: on some platforms this triggers `ScaleFactorChanged`; on others it does not until the next resize. Document the difference in code comments.
- [ ] 9.3. Multiple adapters: if the user has both integrated and discrete GPU, prefer discrete for `PowerPreference::HighPerformance` when hardware is plugged in. Document the choice; add an env var `IDE_POWER_PREFERENCE=low|high` to override.
- [ ] 9.4. Surface loss during sleep/wake: add a recovery path that re-acquires after a reasonable wait.
- [ ] 9.5. Commit: `fix(render): handle minimize, display moves, and surface loss edge cases`.

### 10. Benchmarks for render loop overhead

- [ ] 10.1. `crates/editor-render/benches/frame_overhead.rs`: benchmark `render_frame` on a tiny headless surface (1×1). Measures the overhead of the frame loop itself, minus GPU work. Target: well under 1ms.
- [ ] 10.2. Save Criterion baseline as `m03-mvp`.
- [ ] 10.3. Commit: `bench(render): add frame-overhead benchmark`.

### 11. Cross-platform verification

- [ ] 11.1. On Windows: confirm `Backend: Dx12` in logs. Drag window between 1x and 2x monitors. Resize rapidly. No flicker, no crash.
- [ ] 11.2. On Linux (via CI + manual if possible): confirm `Backend: Vulkan` or fallback GL. `--dry-run` exits 0.
- [ ] 11.3. On macOS (via CI + manual if possible): confirm `Backend: Metal`. Resize works. Retina scaling works.
- [ ] 11.4. If any OS fails, document the issue in `/docs/CROSS_PLATFORM.md` and fix it before proceeding.
- [ ] 11.5. Commit: `fix(render): cross-platform adjustments for DX12/Metal/Vulkan`.

### 12. Quality gates

- [ ] 12.1. `cargo fmt --all --check`.
- [ ] 12.2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] 12.3. `cargo test --workspace`.
- [ ] 12.4. `cargo bench -p editor-render --no-run`.
- [ ] 12.5. `cargo run --release --bin editor-app` — visually confirm animated clear color. Resize. Close. Exit 0.
- [ ] 12.6. CI green on all three OSes.

### 13. Documentation

- [ ] 13.1. Update `/docs/RENDERING_PIPELINE.md` with what's actually implemented.
- [ ] 13.2. Update `/docs/ARCHITECTURE.md` if the crate API changed from the M00 sketch.
- [ ] 13.3. Update `/docs/STATUS.md`: M03 complete, M04 next.
- [ ] 13.4. Update `/CHANGELOG.md`:
  ```
  ### Added
  - `editor-render::Renderer` with robust surface management, backend fallback, frame timing.
  - DX12 on Windows, Metal on macOS, Vulkan on Linux, with fallback paths.
  - `--dry-run` headless mode for CI.
  ```
- [ ] 13.5. Tag: `git tag -a m03-complete -m "M03 complete: wgpu rendering foundation"`; push tag.

---

## Validation / Acceptance Criteria

M03 is complete when:

1. Quality gates pass on Windows.
2. CI is green on all three OSes.
3. `cargo run --release --bin editor-app` shows a window with an animated clear color that reacts smoothly to resize.
4. Running with `RUST_LOG=editor_render=debug` shows frame times around 16 ms (60 fps) when `PresentMode::FifoRelaxed` is active.
5. Running with `IDE_PRESENT_MODE=immediate` shows much smaller frame times (capped by hardware).
6. `--dry-run` mode exits 0 on all three OSes.
7. Benchmarks captured as `m03-mvp` baseline.
8. `docs/STATUS.md` reflects "M03 done, M04 next."
9. `m03-complete` tag pushed.

## Testing Requirements

- Unit tests for `FrameTimer` statistics.
- Integration test for `SurfaceManager` reconfigure sequence.
- `--dry-run` smoke test in CI.
- Manual smoke: open, resize, close on Windows.
- Benchmarks for render-loop overhead.

## Git Commit Strategy

10-14 commits. Push after items 3, 6, 7, 9, 11, 13.

## Handoff to M04

M04 assumes:

- `editor-render::Renderer` exists with `render_frame(color)` and exposes `gpu()`, `surface_format()`, `surface_config()`.
- M04 adds a `TextLayer` subsystem that uses `glyphon` + `cosmic-text`, attaches to the renderer's existing render pass (middleware pattern), and draws the visible portion of a `TextBuffer`.

---

## Standing Orders Reminder

- wgpu errors are not panics; they are handleable. No `.unwrap()` on GPU calls except in one-shot init paths where a clean error-propagation path is equivalent.
- Resize must never crash. Test aggressively: resize to 1x1, resize while the mouse drags rapidly, resize across DPI changes.
- Frame time > 16ms is a warning-level log event (in debug builds; release builds stay quiet unless it's consistently > 50ms). This is our earliest performance feedback loop.
- Do not pin to a breaking wgpu version without updating `/docs/TECH_STACK.md` to match.

Go.
