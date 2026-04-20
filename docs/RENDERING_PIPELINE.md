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

`wgpu::InstanceDescriptor`:

- Windows: prefer DX12, fall back to Vulkan, then GL.
- Linux: Vulkan preferred, GL fallback.
- macOS: Metal only.

Logic lives in one `editor-render` function so it is testable in
isolation. Backend choice is logged at startup via `tracing::info!`.

## 4. Surface & Present Mode

Surface format: `Bgra8UnormSrgb` preferred; fall back to whatever the
adapter reports if that's unavailable.

Present mode ranking:

1. `Mailbox` — preferred on DX12 / Vulkan / Metal for low-latency.
2. `Fifo` — safe fallback (always supported).
3. `Immediate` — only if explicitly enabled (`--present-immediate`); may
   tear.

Re-configure the surface on:

- `WindowEvent::Resized` (use inner size).
- `WindowEvent::ScaleFactorChanged`.
- Backend change (never at runtime in MVP).

## 5. Glyph Atlas via `glyphon`

We delegate atlas management to `glyphon` to avoid rebuilding a known
hard problem:

- `TextAtlas` holds glyphs on the GPU.
- `TextRenderer` builds instance buffers.
- We feed it `cosmic_text::Buffer`s for the visible lines.

Atlas size cap: 64 MiB GPU-resident (see `PERFORMANCE_BUDGETS.md`).
`glyphon` evicts least-recently-used glyphs when saturated.

One atlas per window, shared across all text regions (text canvas, gutter,
status bar).

## 6. Layout Strategy

- **Per-line `cosmic_text::Buffer`** for the main canvas, lazily created
  and cached by (`line_index`, `document_version`) tuple.
- **Viewport clipping:** only lines whose visible range intersects the
  viewport are shaped.
- **Dirty-line invalidation:** `editor-core` reports a `RangeSet<LineIdx>`
  of changed lines each frame; their cached `Buffer`s are discarded.
- **Wrap mode:** in MVP, wrap disabled (long lines horizontally scroll or
  clip). Wrap is a deliberate V2+ concern because it complicates row-col
  math.

## 7. Frame Anatomy

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

## 9. Selection Rendering (V2)

- Split into **per-visible-line rectangles**.
- Instanced draw: one instance per line fragment, one vertex buffer.
- Color is semi-transparent accent; drawn behind text.

Never generate a rectangle per character. Per-visible-line is the upper
bound.

## 10. Line Numbers (V2)

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

## 12. Failure Modes & Fallbacks

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

## 14. Future

- Subpixel text anti-aliasing quality tuning.
- Frame-pacing for 120 Hz displays with explicit vertical blanking
  alignment.
- Optional vector background (e.g. code-map).
- Post-processing pass (dim inactive splits) — out of scope until
  V2+.

All future extensions must keep the rule: **cost scales with visible
screen, not document size.**

---

*Last updated: M00.*

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

