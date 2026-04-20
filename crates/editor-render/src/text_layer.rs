//! GPU text via glyphon + cosmic-text (M04).

use std::cmp::min;

use editor_core::{ScrollOffset, TextBufferSnapshot};
use editor_ui::StatusBarLayout;
use glyphon::{
    Attrs, Buffer, Cache, Color, ColorMode, ContentType, CustomGlyph, Family, FontSystem, Metrics,
    PrepareError, RasterizeCustomGlyphRequest, RasterizedCustomGlyph, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use tracing::warn;
use wgpu::{Device, MultisampleState, Queue, RenderPass, TextureFormat};
use winit::dpi::PhysicalSize;

use crate::error::RenderError;

/// Bundled monospace so the editor renders without relying on system font installs (M04).
const BUNDLED_JETBRAINS_MONO: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
const BUNDLED_MONO_FAMILY: &str = "JetBrains Mono";

const CURSOR_GLYPH_ID: u16 = 1;

fn rasterize_cursor(req: RasterizeCustomGlyphRequest) -> Option<RasterizedCustomGlyph> {
    if req.id != CURSOR_GLYPH_ID {
        return None;
    }
    let n = req.width as usize * req.height as usize;
    Some(RasterizedCustomGlyph { data: vec![255u8; n], content_type: ContentType::Mask })
}

fn build_text_areas<'a>(
    line_buffers: &'a [Buffer],
    custom_glyphs_per_line: &'a [Vec<CustomGlyph>],
    first: usize,
    physical_size: PhysicalSize<u32>,
    scroll: ScrollOffset,
    line_h: f32,
    clip_bottom: i32,
) -> Vec<TextArea<'a>> {
    let w = physical_size.width as i32;
    (0..line_buffers.len())
        .map(|i| {
            let line_idx = first + i;
            TextArea {
                buffer: &line_buffers[i],
                left: 8.0,
                top: (line_idx as f32) * line_h - scroll.y_px + 4.0,
                scale: 1.0,
                bounds: TextBounds { left: 0, top: 0, right: w, bottom: clip_bottom },
                default_color: Color::rgb(0xE0, 0xE0, 0xE0),
                custom_glyphs: &custom_glyphs_per_line[i],
            }
        })
        .collect()
}

fn push_dev_hud_text_area<'a>(
    hud_buffer: Option<&'a Buffer>,
    physical_size: PhysicalSize<u32>,
    clip_bottom: i32,
    areas: &mut Vec<TextArea<'a>>,
) {
    if let Some(buf) = hud_buffer {
        let w = physical_size.width as i32;
        let left = (physical_size.width as f32 - 520.0).max(8.0);
        areas.push(TextArea {
            buffer: buf,
            left,
            top: 6.0,
            scale: 1.0,
            bounds: TextBounds { left: 0, top: 0, right: w, bottom: clip_bottom },
            default_color: Color::rgb(0x90, 0xD0, 0x70),
            custom_glyphs: &[],
        });
    }
}

/// Renders visible lines from a [`TextBufferSnapshot`] into an existing wgpu pass.
pub struct TextLayer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    /// Must outlive [`Viewport`] / [`TextAtlas`] bind groups.
    #[allow(dead_code)]
    cache: Cache,
    atlas: TextAtlas,
    viewport: Viewport,
    text_renderer: TextRenderer,
    line_buffers: Vec<Buffer>,
    custom_glyphs_per_line: Vec<Vec<CustomGlyph>>,
    /// Bottom status line (retained so [`TextArea`] can borrow it for one frame).
    status_line_buffer: Option<Buffer>,
    /// Top-right dev HUD (F11 metrics overlay).
    dev_hud_buffer: Option<Buffer>,
    scale_factor: f32,
}

impl std::fmt::Debug for TextLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextLayer").finish_non_exhaustive()
    }
}

impl TextLayer {
    /// Build atlas, pipelines, and load bundled JetBrains Mono plus system fallbacks.
    pub fn new(device: &Device, queue: &Queue, surface_format: TextureFormat) -> Self {
        let mut font_system = FontSystem::new();
        font_system.db_mut().load_font_data(BUNDLED_JETBRAINS_MONO.to_vec());
        font_system.db_mut().load_system_fonts();

        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas =
            TextAtlas::with_color_mode(device, queue, &cache, surface_format, ColorMode::Accurate);
        let viewport = Viewport::new(device, &cache);
        let text_renderer =
            TextRenderer::new(&mut atlas, device, MultisampleState::default(), None);

        Self {
            font_system,
            swash_cache,
            cache,
            atlas,
            viewport,
            text_renderer,
            line_buffers: Vec::new(),
            custom_glyphs_per_line: Vec::new(),
            status_line_buffer: None,
            dev_hud_buffer: None,
            scale_factor: 1.0,
        }
    }

    pub fn set_scale_factor(&mut self, scale: f32) {
        self.scale_factor = scale;
    }

    #[must_use]
    pub fn line_height_px(&self) -> f32 {
        20.0 * self.scale_factor
    }

    pub fn after_frame(&mut self) {
        self.atlas.trim();
    }

    #[allow(clippy::too_many_arguments)] // Glyph layout needs line range + cursor metrics together.
    fn fill_visible_lines(
        &mut self,
        snapshot: &TextBufferSnapshot,
        physical_size: PhysicalSize<u32>,
        cursor_blink_on: bool,
        first: usize,
        last: usize,
        metrics: Metrics,
        line_h: f32,
        cursor_line: usize,
        cursor_col: usize,
    ) {
        let rope = snapshot.rope();
        let attrs = Attrs::new().family(Family::Name(BUNDLED_MONO_FAMILY));

        self.line_buffers.clear();
        self.custom_glyphs_per_line.clear();

        for line_idx in first..last {
            let mut buf = Buffer::new(&mut self.font_system, metrics);
            buf.set_size(&mut self.font_system, Some(physical_size.width as f32), None);
            let line_text = rope.line(line_idx).to_string();
            buf.set_text(&mut self.font_system, &line_text, &attrs, Shaping::Advanced, None);
            buf.shape_until_scroll(&mut self.font_system, false);

            let mut customs = Vec::new();
            if cursor_blink_on && line_idx == cursor_line {
                let cw = metrics.font_size * 0.6;
                let x = cursor_col as f32 * cw;
                customs.push(CustomGlyph {
                    id: CURSOR_GLYPH_ID,
                    left: x,
                    top: 0.0,
                    width: 2.0,
                    height: line_h,
                    color: Some(Color::rgb(0xEE, 0xEE, 0xEE)),
                    snap_to_physical_pixel: true,
                    metadata: 0,
                });
            }
            self.line_buffers.push(buf);
            self.custom_glyphs_per_line.push(customs);
        }
    }

    fn set_dev_hud_buffer(&mut self, dev_hud_line: Option<&str>) {
        let hud_metrics = Metrics::new(11.0 * self.scale_factor, 15.0 * self.scale_factor);
        self.dev_hud_buffer = None;
        if let Some(text) = dev_hud_line.filter(|s| !s.is_empty()) {
            let attrs = Attrs::new().family(Family::Name(BUNDLED_MONO_FAMILY));
            let mut hud_buf = Buffer::new(&mut self.font_system, hud_metrics);
            hud_buf.set_size(&mut self.font_system, Some(520.0), None);
            hud_buf.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
            hud_buf.shape_until_scroll(&mut self.font_system, false);
            self.dev_hud_buffer = Some(hud_buf);
        }
    }

    /// Shape glyphs and upload atlas data. Call before starting the render pass.
    #[allow(clippy::too_many_arguments)] // Matches wgpu + document snapshot inputs.
    pub fn prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        snapshot: &TextBufferSnapshot,
        scroll: ScrollOffset,
        cursor_byte: usize,
        physical_size: PhysicalSize<u32>,
        cursor_blink_on: bool,
        status_bar: Option<&StatusBarLayout>,
        dev_hud_line: Option<&str>,
    ) -> Result<(), RenderError> {
        let rope = snapshot.rope();
        let total_lines = rope.len_lines();
        let metrics = Metrics::new(14.0 * self.scale_factor, 20.0 * self.scale_factor);
        let line_h = metrics.line_height;

        let status_h = status_bar.map(|s| s.height_px).unwrap_or(0.0);
        let content_px = (physical_size.height as f32 - status_h).max(1.0);
        let clip_bottom = content_px.round() as i32;

        self.viewport.update(
            queue,
            Resolution { width: physical_size.width.max(1), height: physical_size.height.max(1) },
        );

        let first = (scroll.y_px / line_h).floor().max(0.0) as usize;
        let visible = (content_px / line_h).ceil() as usize + 2;
        let last = min(first + visible, total_lines);

        let byte = cursor_byte.min(rope.len_bytes());
        let cursor_line = rope.byte_to_line(byte);
        let line_start = rope.line_to_byte(cursor_line);
        let cursor_col = byte - line_start;

        self.fill_visible_lines(
            snapshot,
            physical_size,
            cursor_blink_on,
            first,
            last,
            metrics,
            line_h,
            cursor_line,
            cursor_col,
        );

        self.status_line_buffer = None;
        let attrs = Attrs::new().family(Family::Name(BUNDLED_MONO_FAMILY));
        if let Some(sb) = status_bar {
            let mut sbuf = Buffer::new(&mut self.font_system, metrics);
            sbuf.set_size(&mut self.font_system, Some(physical_size.width as f32), None);
            sbuf.set_text(&mut self.font_system, &sb.line, &attrs, Shaping::Advanced, None);
            sbuf.shape_until_scroll(&mut self.font_system, false);
            self.status_line_buffer = Some(sbuf);
        }

        self.set_dev_hud_buffer(dev_hud_line);

        let mut areas = build_text_areas(
            &self.line_buffers,
            &self.custom_glyphs_per_line,
            first,
            physical_size,
            scroll,
            line_h,
            clip_bottom,
        );

        if let (Some(sb), Some(buf)) = (status_bar, self.status_line_buffer.as_ref()) {
            let w = physical_size.width as i32;
            let h = physical_size.height as i32;
            let clip_top = (physical_size.height as f32 - sb.height_px).max(0.0).round() as i32;
            let top = physical_size.height as f32 - sb.height_px + 4.0;
            areas.push(TextArea {
                buffer: buf,
                left: 8.0,
                top,
                scale: 1.0,
                bounds: TextBounds { left: 0, top: clip_top, right: w, bottom: h },
                default_color: Color::rgb(0xB0, 0xB0, 0xB0),
                custom_glyphs: &[],
            });
        }

        push_dev_hud_text_area(
            self.dev_hud_buffer.as_ref(),
            physical_size,
            clip_bottom,
            &mut areas,
        );

        let prep = self.text_renderer.prepare_with_custom(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            areas,
            &mut self.swash_cache,
            rasterize_cursor,
        );

        match prep {
            Ok(()) => Ok(()),
            Err(PrepareError::AtlasFull) => {
                warn!("glyph atlas full; trimming and retrying once");
                self.atlas.trim();
                self.fill_visible_lines(
                    snapshot,
                    physical_size,
                    cursor_blink_on,
                    first,
                    last,
                    metrics,
                    line_h,
                    cursor_line,
                    cursor_col,
                );
                self.status_line_buffer = None;
                if let Some(sb) = status_bar {
                    let mut sbuf = Buffer::new(&mut self.font_system, metrics);
                    sbuf.set_size(&mut self.font_system, Some(physical_size.width as f32), None);
                    sbuf.set_text(&mut self.font_system, &sb.line, &attrs, Shaping::Advanced, None);
                    sbuf.shape_until_scroll(&mut self.font_system, false);
                    self.status_line_buffer = Some(sbuf);
                }
                self.set_dev_hud_buffer(dev_hud_line);
                let mut areas2 = build_text_areas(
                    &self.line_buffers,
                    &self.custom_glyphs_per_line,
                    first,
                    physical_size,
                    scroll,
                    line_h,
                    clip_bottom,
                );
                if let (Some(sb), Some(buf)) = (status_bar, self.status_line_buffer.as_ref()) {
                    let w = physical_size.width as i32;
                    let h = physical_size.height as i32;
                    let clip_top =
                        (physical_size.height as f32 - sb.height_px).max(0.0).round() as i32;
                    let top = physical_size.height as f32 - sb.height_px + 4.0;
                    areas2.push(TextArea {
                        buffer: buf,
                        left: 8.0,
                        top,
                        scale: 1.0,
                        bounds: TextBounds { left: 0, top: clip_top, right: w, bottom: h },
                        default_color: Color::rgb(0xB0, 0xB0, 0xB0),
                        custom_glyphs: &[],
                    });
                }
                push_dev_hud_text_area(
                    self.dev_hud_buffer.as_ref(),
                    physical_size,
                    clip_bottom,
                    &mut areas2,
                );
                self.text_renderer
                    .prepare_with_custom(
                        device,
                        queue,
                        &mut self.font_system,
                        &mut self.atlas,
                        &self.viewport,
                        areas2,
                        &mut self.swash_cache,
                        rasterize_cursor,
                    )
                    .map_err(|e| RenderError::TextPrepare(e.to_string()))
            }
        }
    }

    pub fn render(&self, pass: &mut RenderPass<'_>) -> Result<(), RenderError> {
        self.text_renderer
            .render(&self.atlas, &self.viewport, pass)
            .map_err(|e| RenderError::TextRender(e.to_string()))
    }
}
