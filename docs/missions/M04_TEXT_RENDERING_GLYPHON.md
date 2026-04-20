# M04 — Text Rendering with glyphon & cosmic-text

**Mission ID:** M04
**Prerequisites:** M03 complete (`editor-render::Renderer` works, window opens with animated clear color).
**Output:** A `TextLayer` subsystem inside `editor-render` that uses `glyphon` + `cosmic-text` to draw the visible portion of a `TextBuffer`'s content into the existing render pass. The MVP editor window now shows real text. Scrolling works. DPI works. Monospaced font bundled.
**Estimated scope:** 2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/docs/RENDERING_PIPELINE.md` — delta rendering, middleware pattern, glyph atlas.
- `/docs/TEXT_ENGINE.md` — how to read from `TextBuffer` (chunks iterator, line slices).
- `/docs/ARCHITECTURE.md` — the `editor-render`/`editor-core` boundary.
- `/docs/CROSS_PLATFORM.md` — font fallback, DPI, DirectWrite vs core-text vs fontconfig.
- `https://docs.rs/glyphon/latest/glyphon/` — API surface. `TextAtlas`, `TextRenderer`, `Viewport`, `TextArea`, `Cache`.
- `https://github.com/grovesNL/glyphon` — README examples.
- `https://github.com/pop-os/cosmic-text` — shaping, layout, `FontSystem`, `SwashCache`.

Before coding, **verify glyphon's current version on crates.io** and confirm it matches the wgpu major version pinned in `editor-render`. A glyphon-wgpu version skew is the single most common way M04 stalls.

---

## The Situation In Plain English

The window currently renders an animated clear color. That proves the GPU pipeline works but doesn't yet look like an editor. M04 takes the `TextBuffer` from `editor-core` and renders its content on the screen, line by line, only for the portion that is actually visible. Everything below the viewport never hits the CPU rasterizer or the GPU atlas. This is the key performance property that lets us stay fast on a 100 MB file.

The library we use, `glyphon`, is deliberately small and unopinionated. It is *middleware* — it does not own the render pass, the surface, or the present loop. It integrates *into* an existing render pass as a separate pipeline that draws glyph quads. That is exactly what we want: our `Renderer` already owns the render pass, and `TextLayer` just adds draws to it during the same frame. No extra passes, no state change thrashing, minimal overhead.

Under the hood, glyphon delegates CPU-side text work to `cosmic-text`. Cosmic-text handles the hard parts: font enumeration via `fontdb` (which uses DirectWrite on Windows, Core Text on macOS, Fontconfig on Linux), shaping via `harfrust` (a pure-Rust HarfBuzz port), line breaking, bidirectional text, grapheme segmentation, and rasterization via `swash`. The rasterized glyphs get packed into a GPU texture atlas by `etagere`'s shelf-packing algorithm, and then drawn as instanced quads.

Our responsibilities in M04 are much narrower:

1. Create a `FontSystem` and a `SwashCache`, both owned by `TextLayer`.
2. Bundle a default monospace font (JetBrains Mono, Fira Code, or similar — permissively licensed) so the editor always has something to render even on systems with no monospace fonts installed.
3. On every frame, inspect the current `TextBuffer`, figure out what lines are visible in the viewport given the current scroll offset, build `cosmic-text::Buffer`s (or reuse them — we'll cache), and pass them to glyphon's `prepare` method.
4. Call glyphon's `render` inside the existing render pass of `Renderer`.
5. Handle DPI: scale factor changes must re-lay-out and re-rasterize at the new size.
6. Draw a visible cursor (a thin vertical rectangle at the cursor's position) — this is the first non-text primitive and proves our pipeline can handle custom quads next to text.

We do *not* do syntax highlighting, soft wrapping, multi-cursors, selection highlights, gutter rendering, or any of the other things that look like "editor features." Those belong to later missions. M04 proves the rendering loop works end-to-end for plain text.

---

## Scope

**In scope:**
- `editor-render::TextLayer` owning `FontSystem`, `SwashCache`, `TextAtlas`, `TextRenderer`, `Viewport`, and `Cache`.
- Bundled default monospace font.
- Visible-viewport computation (scroll offset + viewport height → first/last visible line).
- Cursor rendering as a thin vertical quad.
- DPI-aware metrics (font size in logical points, scaled to physical pixels).
- Integration into `Renderer::render_frame` via the middleware pattern.
- `editor-app` now owns a `TextBuffer` + a scroll offset + a cursor and passes them to the renderer.
- Basic scroll (PageUp/PageDown, Up/Down arrows move the scroll offset as well as the cursor). Full input handling comes in M05, but we need enough to visually validate scrolling works.
- Opening a test fixture file (hard-coded path for now; real file I/O is M06) to have something substantial to render.

**Out of scope:**
- Syntax highlighting.
- Selection highlighting (M09).
- Soft wrapping (V2+).
- Line numbers gutter (M09).
- Multi-cursor (post-V2).
- Hover cursor / mouse text selection (M09).
- LSP, AI, any language awareness.

---

## North Star

At the end of M04, `cargo run --release --bin editor-app -- path/to/a/file.txt` (or with a hardcoded fixture) opens a window that:

- Displays the file's text in a clean monospaced font.
- Shows a blinking cursor at a fixed position (initially byte 0).
- Scrolls smoothly when the user presses arrow keys / PageUp/PageDown.
- Resizes without layout glitches.
- Scales correctly when dragged between 1x and 2x monitors.
- Maintains ≥60 fps during continuous scroll on a 10 MB file.

---

## TODO List

### 1. Add glyphon to `editor-render`'s deps and bundle a default font

- [ ] 1.1. `cargo add -p editor-render glyphon` (verify the version matches the pinned `wgpu` major; update `Cargo.toml` in the workspace if needed). Add `cosmic-text` as a direct dep too for convenience (even though glyphon re-exports the parts we need, being explicit aids clarity).
- [ ] 1.2. Pick a default bundled monospace font. Recommended: **JetBrains Mono** (Apache-2.0, free, excellent on-screen). Alternative: **Fira Code** (OFL-1.1). Download the `.ttf` or `.otf` for Regular weight (and optionally Bold) and commit it under `crates/editor-render/assets/fonts/`. Include the font's LICENSE next to it.
- [ ] 1.3. Embed it at compile time with `include_bytes!(...)` so the font travels with the binary. No runtime dependency on system-installed fonts required.
- [ ] 1.4. Document the choice in `/docs/RENDERING_PIPELINE.md` under a "Default font" heading and in `/docs/TECH_STACK.md`.
- [ ] 1.5. Commit: `feat(render): add glyphon/cosmic-text deps and bundle JetBrains Mono Regular`.

### 2. Design the `TextLayer` API

- [ ] 2.1. Open `/docs/API_DESIGN_NOTES.md` (or wherever you captured M02's design). Sketch:
  ```rust
  pub struct TextLayer {
      font_system: FontSystem,
      swash_cache: SwashCache,
      atlas: TextAtlas,
      text_renderer: TextRenderer,
      viewport: Viewport,
      cache: Cache,
      /// Cosmic-text `Buffer`s keyed by "line range visible in this frame".
      /// Recomputed on content change; reused across frames when the viewport
      /// and buffer are unchanged.
      layout_cache: LayoutCache,
      metrics: Metrics,  // font size + line height
      scale_factor: f32,
  }

  impl TextLayer {
      pub fn new(gpu: &GpuHandle, surface_format: TextureFormat, scale_factor: f32) -> Self;
      pub fn set_scale_factor(&mut self, new_scale: f32);
      pub fn set_metrics(&mut self, font_size: f32, line_height: f32);
      /// Called every frame before render. Computes visible lines, updates atlas.
      pub fn prepare(
          &mut self,
          gpu: &GpuHandle,
          buffer: &TextBufferSnapshot,
          scroll: ScrollOffset,
          viewport_size: PhysicalSize<u32>,
      ) -> Result<(), RenderError>;
      /// Called inside the Renderer's render pass.
      pub fn render<'a>(&'a self, pass: &mut RenderPass<'a>) -> Result<(), RenderError>;
  }
  ```
- [ ] 2.2. `ScrollOffset` is a new tiny type in `editor-core` (or maybe `editor-render`; put it in `editor-core` because scrolling is a conceptual property of viewing a buffer, not a render-specific one). It holds a y-offset in pixels. Later missions may grow it to include x-offset for horizontal scrolling.
- [ ] 2.3. `LayoutCache` is an LRU-style cache keyed by `(line_index, version)` → `cosmic_text::Buffer` — it reuses laid-out lines when the document content and wrap width haven't changed. For MVP, a naïve `HashMap` is fine; we can LRU-evict when it exceeds some size (e.g., 2× visible line count).
- [ ] 2.4. Commit: `docs(render): sketch TextLayer API`.

### 3. Implement `TextLayer::new`

- [ ] 3.1. Create `FontSystem`:
  ```rust
  let mut font_system = FontSystem::new();
  font_system.db_mut().load_font_data(BUNDLED_FONT_BYTES.to_vec());
  ```
  This inserts the bundled font into `fontdb`'s in-memory list. Optionally also call `db_mut().load_system_fonts()` to pick up user-installed fonts as fallbacks (this can be slow the first time on macOS — consider running it on a background thread if it becomes an issue; for M04 synchronous is fine).
- [ ] 3.2. Create `SwashCache::new()`, `Cache::new(device)`, `TextAtlas::new(device, queue, &cache, surface_format)`, `Viewport::new(device, &cache)`, `TextRenderer::new(&mut atlas, device, MultisampleState::default(), None)`.
- [ ] 3.3. Initialize `metrics` to sensible defaults: font size 14.0, line height 20.0. These are logical pixels. They get multiplied by `scale_factor` when passed to cosmic-text.
- [ ] 3.4. Unit test (behind `#[cfg(test)]`): construct a `TextLayer` against a headless wgpu device. Confirm no panics.
- [ ] 3.5. Commit: `feat(render): implement TextLayer::new with bundled font and cosmic-text init`.

### 4. Implement `TextLayer::prepare`

- [ ] 4.1. Compute the visible line range:
  ```rust
  let line_height_px = self.metrics.line_height * self.scale_factor;
  let first_visible_line = (scroll.y_px / line_height_px).floor() as usize;
  let visible_line_count = ((viewport_size.height as f32) / line_height_px).ceil() as usize + 1;
  let last_visible_line = (first_visible_line + visible_line_count).min(buffer.len_lines());
  ```
- [ ] 4.2. For each visible line, either reuse a cached `cosmic_text::Buffer` or build a new one. When building: create a `Buffer` with `Metrics::new(font_size * scale, line_height * scale)`, set its size to the viewport width (unbounded height — we only lay out one line at a time), call `set_text` with the line's string content and a default `Attrs` pointing at the monospace family.
- [ ] 4.3. Call `buffer.shape_until_scroll(&mut font_system, /*prune=*/true)` on each per-line Buffer to trigger shaping/layout.
- [ ] 4.4. Build a `Vec<TextArea>` for the visible lines. Each `TextArea` references one of the per-line `Buffer`s, with its position computed from the line index minus the scroll offset:
  ```rust
  let y_pixels = (line_index as f32) * line_height_px - scroll.y_px;
  TextArea {
      buffer: &per_line_buffer,
      left: 0.0,
      top: y_pixels,
      scale: 1.0,  // scale factor already baked into metrics
      bounds: TextBounds { left: 0, top: 0, right: viewport_size.width as i32, bottom: viewport_size.height as i32 },
      default_color: Color::rgb(0xE0, 0xE0, 0xE0),
      custom_glyphs: &[],
  }
  ```
- [ ] 4.5. Update the glyphon `Viewport` with the current `Resolution` (physical viewport size).
- [ ] 4.6. Call `self.text_renderer.prepare(device, queue, &mut self.font_system, &mut self.atlas, &mut self.viewport, text_areas, &mut self.swash_cache)`. This is where glyphon actually uploads glyph bitmaps to the atlas and builds vertex data.
- [ ] 4.7. Error handling: `PrepareError::AtlasFull` is possible if the atlas fills up. On that error, call `atlas.trim()` and retry once; if that still fails, log at `error!` and skip this frame's text render rather than crashing.
- [ ] 4.8. Commit: `feat(render): implement TextLayer::prepare with visible-line computation and layout cache`.

### 5. Implement `TextLayer::render`

- [ ] 5.1. Call `self.text_renderer.render(&self.atlas, &self.viewport, pass)`. That's it — glyphon does the draw call.
- [ ] 5.2. Add rustdoc explaining this must be called inside a live `RenderPass` that was configured with the same surface format passed to `TextLayer::new`.
- [ ] 5.3. Commit: `feat(render): implement TextLayer::render (middleware draw into existing pass)`.

### 6. Wire `TextLayer` into `Renderer`

- [ ] 6.1. `Renderer` gains a `text_layer: TextLayer` field.
- [ ] 6.2. `Renderer::new` constructs it after the surface format is known.
- [ ] 6.3. Change the `Renderer::render_frame` signature to accept what it needs to draw:
  ```rust
  pub struct FrameInput<'a> {
      pub buffer: &'a TextBufferSnapshot,
      pub scroll: ScrollOffset,
      pub cursor: Cursor,
      pub clear_color: wgpu::Color,
  }
  pub fn render_frame(&mut self, input: FrameInput<'_>) -> Result<(), RenderError>;
  ```
- [ ] 6.4. Inside `render_frame`: call `text_layer.prepare(...)` **before** acquiring the surface texture (prepare does CPU work that can happen while we wait for the previous frame to present). Then: acquire texture → begin render pass → `text_layer.render(&mut pass)` → end pass → submit → present.
- [ ] 6.5. On resize, `Renderer::resize` also calls `text_layer.set_viewport_size(new_size)` if we choose to cache it, or we just pass the new size to `prepare` next frame.
- [ ] 6.6. On scale factor change, `Renderer::on_scale_factor_change` calls `text_layer.set_scale_factor(new_scale)` which invalidates the layout cache.
- [ ] 6.7. Commit: `feat(render): integrate TextLayer into Renderer::render_frame`.

### 7. Render the cursor

- [ ] 7.1. Add a second tiny pipeline in `editor-render` for drawing solid-colored quads. Call it `QuadLayer`. It has a simple WGSL shader (vertex + fragment) that takes a position, size, and color and draws a rectangle.
- [ ] 7.2. Keep this pipeline minimal: 1 bind group for the viewport (reusing glyphon's viewport bind group is an option but adds coupling; keep a separate one for now), instanced draws where each instance is a quad.
- [ ] 7.3. The cursor becomes a 2-pixel-wide quad at the cursor's line/column position in physical pixels. Compute its position from `(cursor.pos, buffer, metrics, scroll)`: byte → line/col, line/col → x/y pixel.
- [ ] 7.4. For line/col → pixel x conversion: measure the line's text up to the column using cosmic-text (there's a method on `Buffer` to find the x-offset of a character index) or for the MVP just assume uniform monospace width (`col * advance_width`). This second path is fast and good enough while we're monospaced-only.
- [ ] 7.5. Blink: toggle visibility based on `Instant::now().elapsed_since_app_start() / 500ms % 2`. Skip the draw when "off". This is purely cosmetic but feels right.
- [ ] 7.6. Integrate `QuadLayer::render(&mut pass)` after `TextLayer::render` so the cursor draws on top.
- [ ] 7.7. Commit: `feat(render): add QuadLayer and draw a blinking cursor`.

### 8. Hook `editor-app` to display a real text buffer

- [ ] 8.1. `editor-app`'s `App` struct grows:
  ```rust
  struct App {
      window: Option<Arc<Window>>,
      renderer: Option<Renderer>,
      buffer: TextBuffer,
      cursor: Cursor,
      scroll: ScrollOffset,
      start_time: Instant,
  }
  ```
- [ ] 8.2. On `resumed`, load a fixture file: either accept a CLI argument (`env::args().nth(1)`) and read it with `std::fs::read_to_string` (synchronous for this mission — async I/O is M06), or fall back to a bundled `assets/sample.txt` containing 1000 lines of Lorem Ipsum or similar. Store the content in `self.buffer` via `TextBuffer::from_str`.
- [ ] 8.3. On `RedrawRequested`, build a `FrameInput` and call `renderer.render_frame(input)`. Request another redraw to keep the loop ticking (as in M03).
- [ ] 8.4. On `KeyboardInput` (we're doing minimal handling here — full system in M05):
  - `ArrowUp` / `ArrowDown` → move cursor up/down.
  - `PageUp` / `PageDown` → adjust `scroll.y_px` by one viewport height.
  - `Home` / `End` → move cursor to line start/end.
  - `Escape` → exit.
  Clamp `scroll.y_px` to `0..=max_scroll` where `max_scroll = total_doc_height - viewport_height`.
- [ ] 8.5. On cursor move, also ensure the cursor stays visible in the viewport (auto-scroll logic): if the cursor is above `scroll`, set `scroll = cursor.y_pixel`; if below `scroll + viewport_height`, set `scroll = cursor.y_pixel - viewport_height + line_height`.
- [ ] 8.6. Commit: `feat(app): load fixture file, render real text, navigate with arrows/PageUp/PageDown`.

### 9. Handle edge cases

- [ ] 9.1. Empty buffer: don't try to render anything; skip `text_layer.prepare` cleanly (no crash, no atlas churn).
- [ ] 9.2. Very long single line: cosmic-text will shape the entire line. For MVP we don't soft-wrap, so a long line extends past the right edge and gets clipped by `TextBounds`. This is correct behavior.
- [ ] 9.3. Unicode-heavy content: test with a file containing CJK characters, emoji, RTL text (Arabic/Hebrew). Cosmic-text handles shaping; our job is only to pass the string through and measure widths correctly.
- [ ] 9.4. Font fallback: if the default bundled font doesn't have a glyph for a character, cosmic-text falls back to other fonts in `fontdb`. If we didn't call `load_system_fonts()`, missing glyphs render as a `.notdef` (tofu) box. Decide: enable system font loading on first run (a one-time ~100ms cost) and cache the result, or ship with an emoji font too.
- [ ] 9.5. Scale factor 1.5× (fractional): font metrics must apply `ceil` where necessary to avoid sub-pixel bleed. Test on a 1.5x monitor if possible.
- [ ] 9.6. Window at 0×0 or below our line height: skip rendering.
- [ ] 9.7. Commit: `fix(render): handle empty buffer, long lines, Unicode, fractional DPI, zero-size window`.

### 10. Benchmarks

- [ ] 10.1. `crates/editor-render/benches/text_layer_prepare.rs`: measure `TextLayer::prepare` on a 10 MB buffer with a 1080p viewport showing ~55 lines. Target: < 3 ms per frame.
- [ ] 10.2. Benchmark with cold layout cache (first frame) and warm cache (steady-state scrolling).
- [ ] 10.3. Benchmark atlas growth: render text in N different fonts/sizes and measure atlas upload time.
- [ ] 10.4. Save baseline as `m04-mvp`.
- [ ] 10.5. Commit: `bench(render): add TextLayer::prepare benchmarks`.

### 11. Visual tests

- [ ] 11.1. `crates/editor-render/tests/visual_smoke.rs` (behind `#[ignore]` so CI doesn't run it by default): boot a headless wgpu + an off-screen texture, render a few frames, read back the framebuffer, and sanity-check pixel patterns (e.g., "the top-left 20×20 region has non-background pixels when rendering text there"). Not a full pixel-comparison suite — just smoke. Document how to run manually: `cargo test -p editor-render --test visual_smoke -- --ignored`.
- [ ] 11.2. Commit: `test(render): add ignored visual smoke test for off-screen text rendering`.

### 12. Cross-platform verification

- [ ] 12.1. On Windows: open a file, scroll, resize, drag between monitors of different DPI. Text should stay crisp and readable.
- [ ] 12.2. On macOS (via CI logs): confirm Metal backend + glyphon don't produce format errors. If the surface format is `Bgra8UnormSrgb` but glyphon expects sRGB conversion, tune the `ColorMode` passed to `TextAtlas::new`.
- [ ] 12.3. On Linux (via CI): confirm Vulkan path + glyphon don't produce validation errors. Run `cargo test -p editor-render` in CI.
- [ ] 12.4. Commit: `fix(render): cross-platform color mode and surface format adjustments`.

### 13. Quality gates

- [ ] 13.1. `cargo fmt --all --check`.
- [ ] 13.2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] 13.3. `cargo test --workspace`.
- [ ] 13.4. `cargo bench -p editor-render --no-run`.
- [ ] 13.5. Manual: open a 10 MB text file, scroll continuously for 30 seconds, verify ≥ 60 fps and stable memory via Task Manager (Windows) / Activity Monitor (macOS) / `top` (Linux).

### 14. Documentation

- [ ] 14.1. Update `/docs/RENDERING_PIPELINE.md` with the actual `TextLayer` flow, the layout cache strategy, and the font fallback policy.
- [ ] 14.2. Update `/docs/ARCHITECTURE.md` with the finalized `FrameInput` type.
- [ ] 14.3. Update `/docs/STATUS.md`: M04 complete, M05 next.
- [ ] 14.4. Update `/CHANGELOG.md`:
  ```
  ### Added
  - Text rendering via glyphon + cosmic-text (middleware into Renderer's pass).
  - Bundled JetBrains Mono Regular default font.
  - Visible-viewport-only rendering with per-line layout cache.
  - Basic scroll (PageUp/PageDown, arrow keys).
  - Blinking cursor rendered via QuadLayer.
  ```
- [ ] 14.5. Tag: `git tag -a m04-complete -m "M04 complete: text rendering with glyphon"`; push.

---

## Validation / Acceptance Criteria

M04 is complete when:

1. Quality gates pass on Windows.
2. CI green on all three OSes.
3. `cargo run --release --bin editor-app -- sample.txt` shows readable text in a monospaced font.
4. PageUp/PageDown and arrow keys scroll the view and move the cursor; cursor auto-scrolls into view.
5. Cursor blinks at ~1 Hz.
6. Scrolling a 10 MB file maintains ≥ 60 fps (measured via `FrameTimer`).
7. Window resize and DPI change work without visual glitches.
8. Benchmarks baseline saved as `m04-mvp`.
9. `m04-complete` tag pushed.

## Testing Requirements

- `TextLayer::new` unit test against headless wgpu.
- Ignored visual smoke test.
- Benchmarks for prepare.
- Manual cross-platform verification.

## Git Commit Strategy

12-15 commits. Push after items 4, 7, 8, 10, 12, 14.

## Handoff to M05

M05 assumes:

- `Renderer` accepts a `FrameInput` with buffer + cursor + scroll.
- `TextLayer` is stable and handles the prepare/render cycle.
- `editor-app` has a minimal input loop that proves the pipeline end-to-end.

M05 replaces the ad-hoc input handling in `editor-app` with the real `editor-input` crate, adds IME support, implements the direct-to-state pipeline, and formalizes the frame loop with per-subsystem performance budgets.

---

## Standing Orders Reminder

- Do not widen scope. No syntax highlighting, no gutter, no selection rendering. Those have their own missions.
- Any glyphon or cosmic-text version bump must be accompanied by a confirmation run through all the visual edge cases above.
- If `TextAtlas::prepare` starts returning `AtlasFull` regularly, don't just grow the atlas — investigate whether we're leaking cached glyphs across DPI changes.
- The layout cache is the easiest thing to over-engineer. A naïve `HashMap<(line_index, version), Buffer>` with eviction at 2× visible line count is fine for MVP. We can turn it into an LRU later if benchmarks warrant.

Go.
