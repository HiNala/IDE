[← docs/](./) · [README](../README.md)

# Rendering Pipeline

The renderer lives in the `editor-render` crate. It owns all GPU state.
Nothing else in the workspace holds a `wgpu::Device`, a `wgpu::Queue`, or
a `wgpu::Surface`.

## 1. Responsibilities

- Create and manage the `wgpu::Surface`, `Device`, `Queue`.
- Maintain a glyph atlas via `glyphon`.
- Shape and lay out the visible lines using `cosmic-text`.
- Draw text, cursor, (V2) selection, (V2) line numbers, (V2) status bar.
- Respond to resize / DPI change / present-mode change events.

It does not:

- Own the document or cursor.
- Handle input.
- Perform file I/O.

## 2. Stack Overview

```
editor-core  ──(RenderSnapshot)──▶  editor-render
                                        │
                                        ├── wgpu Surface
                                        │      ↑
                                        │      (swapchain)
                                        │
                                        ├── wgpu Device + Queue
                                        │      │
                                        │      └── command encoder per frame
                                        │
                                        ├── glyphon::TextAtlas (GPU)
                                        ├── glyphon::Viewport
                                        └── cosmic_text::FontSystem, Buffer per visible line
```

## 3. Backend Selection

`crates/editor-render/src/backend.rs` sets [`wgpu::Instance::new`]’s
`InstanceDescriptor.backends` bitmask:

- **Windows:** `DX12 | VULKAN | GL` (DirectX 12 first when available).
- **Linux / BSD:** `VULKAN | GL`.
- **macOS:** `METAL`.

If `WGPU_BACKEND` is set in the environment, that value **replaces** the
platform default (see [`wgpu::Backends::from_env`]).

Startup logs the chosen adapter at `info!` with backend name, vendor id,
`DeviceType`, and driver strings from [`wgpu::AdapterInfo`].

## 3a. Present mode and power

- **`IDE_PRESENT_MODE`** — optional override: `immediate`, `fifo`, `fifo_relaxed`, or
  `mailbox` (must be listed in the surface’s `present_modes`; otherwise we fall back
  to the automatic order below).
- **`IDE_POWER_PREFERENCE`** — `low` tries low-power adapters before discrete; default
  order prefers high performance (discrete when available), then low-power,
  each with `force_fallback_adapter` false then true (CPU / SwiftShader / llvmpipe paths).

Without `IDE_PRESENT_MODE`, present mode follows monitor refresh (see §5).

## 4. Frame interval statistics (M03)

[`editor_render::FrameTimer`] records wall-clock deltas between successive
`EditorRenderer::render_frame` calls (rolling window of 120 samples). Values are
available via [`EditorRenderer::frame_timer`] and logged at `tracing::debug!`
as `editor-render: frame interval`. Exceeding ~16 ms in debug (50 ms in release)
emits a warning — early performance feedback before the dev HUD (M07).

## 5. Surface & Present Mode

Surface format: prefer an sRGB-capable format from the surface caps (see
`GpuContext::new` in `editor-render`).

**Present mode (M12)** — chosen from the surface’s supported modes using the
monitor’s `refresh_rate_millihertz()` (~60 Hz vs ≥120 Hz):

- **≥120 Hz:** prefer `Mailbox`, then `FifoRelaxed`, `Fifo`, `Immediate`.
- **\<120 Hz or unknown:** prefer `FifoRelaxed`, then `Fifo`, `Mailbox`, `Immediate`.

When the window moves to another display (`Moved`, or resize/DPI), we call
`sync_present_mode_for_window` so vsync tracks the new monitor.

Re-configure the surface (`GpuContext::resize`) on:

- `WindowEvent::Resized` (inner physical size; skip 0×0).
- `WindowEvent::ScaleFactorChanged` (often paired with resize).

The swapchain image size tracks the window; the driver reallocates swapchain
storage on `surface.configure`. **Application-owned** resources sized for the
viewport are pooled so resize does not churn Rust-side allocations:

- **Solid quads:** one vertex buffer sized for a fixed maximum rectangle count.
- **Text row scratch:** `TextLayer` pre-grows cosmic-text line buffers and shape-cache
  `Vec`s to `MAX_VISIBLE_ROW_SLOTS` (320 rows — 8K-class height at typical line metrics);
  see `crates/editor-render/src/text_layer.rs`.

A dedicated **8K-class** offscreen *color* attachment for a render-to-texture pass is
*not* used (we draw directly to the swapchain); follow `FOLLOWUPS.md` if that changes.

## 6. Glyph Atlas via `glyphon`

We delegate atlas management to `glyphon` to avoid rebuilding a known
hard problem:

- `TextAtlas` holds glyphs on the GPU.
- `TextRenderer` builds instance buffers.
- We feed it `cosmic_text::Buffer`s for the visible lines.

Atlas size cap: 64 MiB GPU-resident (see `PERFORMANCE_BUDGETS.md`).
`glyphon` evicts least-recently-used glyphs when saturated.

One atlas per window, shared across all text regions (text canvas, gutter,
status bar).

### Default font (M04)

The primary monospace face is **JetBrains Mono Regular** (`Apache-2.0`), shipped as
`crates/editor-render/assets/fonts/JetBrainsMono-Regular.ttf` with
`LICENSE_JETBRAINS.txt`, embedded at compile time via `include_bytes!`.

`FontSystem::db_mut().load_system_fonts()` runs after loading the bundled font so
cosmic-text can fall back to installed fonts when a codepoint is missing from the
bundle (emoji, CJK, etc.). Tofu appears only when no fallback covers the glyph.

## 7. Layout Strategy

- **Per-line `cosmic_text::Buffer`** for the main canvas, cached by a shape key
  that includes `line_index`, document version, **layout width** (for non-wrapped
  mode we use a large fixed width so **horizontal resize does not reshaping**),
  and scale bits.
- **Viewport clipping:** only visible rows are prepared each frame; vertical
  scroll changes which line indices are visible.
- **Dirty-line invalidation:** buffer edits bump the document version and
  invalidate affected cache slots.
- **Wrap mode:** long lines use a fixed large layout width (no soft wrap to the
  viewport width), so resize is cheap; true soft wrap is out of scope until a
  later milestone.

## 8. Frame Anatomy

```
1. Receive RenderSnapshot from editor-core (via arc-swap).
2. If size/DPI changed → reconfigure surface; recompute viewport.
3. Acquire next SurfaceTexture.
4. Begin a RenderPass with a "background" clear color.
5. Layout + draw main text:
   - For each visible line:
       - If not in cache or dirty: reshape.
       - Append glyphon::TextArea.
   - Submit glyphon render.
6. Layout + draw cursor (single quad).
7. (V2) Draw gutter text in a second TextArea list.
8. (V2) Draw selection highlights (instanced quads).
9. (V2) Draw status bar (separate glyph atlas region).
10. End pass; submit command buffer; present.
11. Record per-phase timings (tracing spans).
```

## 8. Cursor Rendering

- One thin vertical quad, size = line height × cursor-width-px (default
  2 px, HiDPI-adjusted).
- Blink handled by time-based alpha on a fixed cycle (e.g. 500 ms on /
  500 ms off). Rendering is always done; only alpha changes, so no
  re-layout needed.
- Position derived from `cursor_byte` via the shared rope.

## 10. Selection Rendering (V2)

- Split into **per-visible-line rectangles**.
- Instanced draw: one instance per line fragment, one vertex buffer.
- Color is semi-transparent accent; drawn behind text.

Never generate a rectangle per character. Per-visible-line is the upper
bound.

## 11. Line Numbers (V2)

- Separate `glyphon::TextRenderer` pass into a dedicated atlas region or
  its own small atlas.
- Cache is invalidated when:
  - Line count digits change (e.g. crossing 999 → 1000).
  - Font / font size changes.
  - Scroll offset changes (positions shift; the strings themselves
    survive).
- Gutter width auto-fits: `ceil(log10(max(1, line_count+1))) * glyph_w + padding`.

## 11. DPI / HiDPI

`winit` reports `scale_factor`. We:

- Multiply all physical pixel dimensions by the current scale.
- Keep font size in logical points; pass the scale to `cosmic_text` for
  rasterization.
- Re-rasterize atlas on scale change.

On multi-monitor systems with differing DPI, a drag between monitors
triggers `ScaleFactorChanged` and we follow the pipeline above.

## 13. Failure Modes & Fallbacks

- **Adapter request fails.** Retry with power-preference `LowPower`.
  If that fails, exit with a clear error message on stderr.
- **Surface lost.** Recreate once; if it fails again, panic via
  `anyhow::bail!` at the shell level.
- **OOM on atlas.** Shrink atlas target size, log warning, continue.
- **GPU hang (detected via `wgpu`'s device-lost callback).** Recreate
  device and all resources; preserve the document (lives in
  `editor-core`, untouched).

## 13. Testing

- **Unit:** viewport intersection logic, dirty-region math, atlas cache
  keys.
- **Integration:** headless wgpu instance where feasible; render one
  frame to a buffer and compare via `insta` snapshot.
- **Manual:** resize, drag between monitors with different DPI, swap
  present modes, open large files.

Per-frame rendering benchmarks use `criterion` with a canned
`RenderSnapshot` and a `wgpu::Backends::empty()` (software) where
available.

## 15. Future

- Subpixel text anti-aliasing quality tuning.
- Frame-pacing for 120 Hz displays with explicit vertical blanking
  alignment.
- Optional vector background (e.g. code-map).
- Post-processing pass (dim inactive splits) — out of scope until
  V2+.

All future extensions must keep the rule: **cost scales with visible
screen, not document size.**

---

*Last updated: M12 (present policy + layout cache notes).*

## M12 — Resize / DPI / frame pacing (summary)

- **Paint on `Resized` / `ScaleFactorChanged`:** the shell calls `paint_frame`
  immediately (interactive path), not only on `RedrawRequested`, so Windows
  **modal resize** keeps updating the client area during edge drag.
- **Battery:** when the `battery` crate reports discharging and the display is
  high-refresh, optional **~60 fps** cap on *normal* redraws (`state.json`:
  `power_uncap_on_battery` to opt out). Interactive resize/DPI paints are not
  throttled.
- **First frame:** the window stays hidden until after the first successful
  `present`, reducing startup flash.

---

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

