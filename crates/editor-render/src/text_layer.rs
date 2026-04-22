//! GPU text via glyphon + cosmic-text (M04).

use std::cmp::min;

use editor_core::{ScrollOffset, TextBufferSnapshot};
use editor_terminal::TerminalRenderSnapshot;
use editor_ui::{FrameChrome, StatusBarLayout};
use glyphon::{
    Attrs, Buffer, Cache, Color, ColorMode, ContentType, CustomGlyph, Family, FontSystem, Metrics,
    PrepareError, RasterizeCustomGlyphRequest, RasterizedCustomGlyph, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use tracing::warn;
use wgpu::{Device, MultisampleState, Queue, RenderPass, TextureFormat};
use winit::dpi::PhysicalSize;

use crate::error::RenderError;

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

/// Line number column width + monospace cell width (matches cursor column spacing).
#[must_use]
pub fn compute_gutter_width_px(total_lines: usize, scale_factor: f32) -> (f32, f32) {
    let metrics = Metrics::new(14.0 * scale_factor, 20.0 * scale_factor);
    let char_w = metrics.font_size * 0.6;
    let digits = total_lines.max(1).to_string().len();
    let gutter_inner = digits as f32 * char_w;
    let gutter_w = gutter_inner + 10.0;
    (gutter_w, char_w)
}

#[allow(clippy::too_many_arguments)]
fn build_layer_text_areas<'a>(
    gutter_buffers: &'a [Buffer],
    line_buffers: &'a [Buffer],
    custom_glyphs_per_line: &'a [Vec<CustomGlyph>],
    first: usize,
    gutter_w: f32,
    physical_size: PhysicalSize<u32>,
    scroll: ScrollOffset,
    line_h: f32,
    clip_bottom: i32,
    content_inset_left_px: f32,
    content_inset_top_px: f32,
) -> Vec<TextArea<'a>> {
    let w = physical_size.width as i32;
    let gutter_left = 8.0 + content_inset_left_px;
    let gutter_right = (gutter_left + gutter_w).round() as i32;
    let body_left = gutter_left + gutter_w;
    let clip_t = content_inset_top_px.max(0.0).round() as i32;
    let clip_l = content_inset_left_px.max(0.0).round() as i32;
    let mut areas = Vec::with_capacity(line_buffers.len() * 2);
    for i in 0..line_buffers.len() {
        let line_idx = first + i;
        let top = (line_idx as f32) * line_h - scroll.y_px + 4.0 + content_inset_top_px;
        areas.push(TextArea {
            buffer: &gutter_buffers[i],
            left: gutter_left,
            top,
            scale: 1.0,
            bounds: TextBounds {
                left: clip_l,
                top: clip_t,
                right: gutter_right,
                bottom: clip_bottom,
            },
            default_color: Color::rgb(0x78, 0x78, 0x78),
            custom_glyphs: &[],
        });
        areas.push(TextArea {
            buffer: &line_buffers[i],
            left: body_left,
            top,
            scale: 1.0,
            bounds: TextBounds { left: 0, top: 0, right: w, bottom: clip_bottom },
            default_color: Color::rgb(0xE0, 0xE0, 0xE0),
            custom_glyphs: &custom_glyphs_per_line[i],
        });
    }
    areas
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

/// Append integrated-terminal [`TextArea`]s; `bufs` must match `snapshot` run count.
#[allow(clippy::too_many_arguments)]
fn push_terminal_text_areas<'a>(
    areas: &mut Vec<TextArea<'a>>,
    bufs: &'a [Buffer],
    snapshot: &TerminalRenderSnapshot,
    physical_size: PhysicalSize<u32>,
    scale_factor: f32,
    status_h: f32,
    terminal_pane_height_px: f32,
    terminal_header_height_px: f32,
    line_h: f32,
    content_inset_left_px: f32,
) {
    if terminal_pane_height_px <= 0.5 || snapshot.rows.is_empty() {
        return;
    }
    let run_count: usize = snapshot.rows.iter().map(|r| r.runs.len()).sum();
    debug_assert_eq!(run_count, bufs.len(), "terminal buffers must match snapshot runs");
    let (gutter_w, char_w) = compute_gutter_width_px(snapshot.rows.len().max(1), scale_factor);
    let body_left = content_inset_left_px + 8.0 + gutter_w;
    let term_top = physical_size.height as f32 - status_h - terminal_pane_height_px
        + terminal_header_height_px
        + 4.0;
    let w = physical_size.width as i32;
    let h = physical_size.height as i32;
    let term_clip_top = term_top.round().max(0.0) as i32;
    let mut idx = 0usize;
    for (row_i, row) in snapshot.rows.iter().enumerate() {
        let top = term_top + row_i as f32 * line_h;
        let mut x = body_left;
        for (text, _fg, _) in &row.runs {
            let buf = &bufs[idx];
            idx += 1;
            areas.push(TextArea {
                buffer: buf,
                left: x,
                top,
                scale: 1.0,
                bounds: TextBounds { left: 0, top: term_clip_top, right: w, bottom: h },
                default_color: Color::rgb(0xe0, 0xe0, 0xe0),
                custom_glyphs: &[],
            });
            x += text.chars().count() as f32 * char_w;
        }
    }
    debug_assert_eq!(idx, bufs.len(), "terminal buffer index mismatch");
}

/// Shape [`FrameChrome`] text lines into buffers and append matching [`TextArea`]s.
#[allow(clippy::too_many_arguments)]
fn push_frame_chrome_text_areas<'a>(
    font_system: &mut FontSystem,
    chrome_overlay_buffers: &'a mut Vec<Buffer>,
    scale_factor: f32,
    frame_chrome: Option<&FrameChrome>,
    physical_size: PhysicalSize<u32>,
    areas: &mut Vec<TextArea<'a>>,
) {
    chrome_overlay_buffers.clear();
    let Some(fc) = frame_chrome else {
        return;
    };
    if fc.lines.is_empty() {
        return;
    }
    let cmetrics = Metrics::new(13.0 * scale_factor, 18.0 * scale_factor);
    for line in &fc.lines {
        let mut buf = Buffer::new(font_system, cmetrics);
        buf.set_size(font_system, Some(physical_size.width as f32), None);
        let attrs = Attrs::new().family(Family::Name(BUNDLED_MONO_FAMILY)).color(Color::rgb(
            line.rgb[0],
            line.rgb[1],
            line.rgb[2],
        ));
        buf.set_text(font_system, &line.text, &attrs, Shaping::Advanced, None);
        buf.shape_until_scroll(font_system, false);
        chrome_overlay_buffers.push(buf);
    }
    let w = physical_size.width as i32;
    let h = physical_size.height as i32;
    for (line, buf) in fc.lines.iter().zip(chrome_overlay_buffers.iter()) {
        areas.push(TextArea {
            buffer: buf,
            left: line.left,
            top: line.top,
            scale: 1.0,
            bounds: TextBounds { left: 0, top: 0, right: w, bottom: h },
            default_color: Color::rgb(line.rgb[0], line.rgb[1], line.rgb[2]),
            custom_glyphs: &[],
        });
    }
}

/// Renders visible lines from a [`TextBufferSnapshot`] into an existing wgpu pass.
pub struct TextLayer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    #[allow(dead_code)]
    cache: Cache,
    atlas: TextAtlas,
    viewport: Viewport,
    text_renderer: TextRenderer,
    line_buffers: Vec<Buffer>,
    custom_glyphs_per_line: Vec<Vec<CustomGlyph>>,
    status_line_buffer: Option<Buffer>,
    dev_hud_buffer: Option<Buffer>,
    gutter_line_buffers: Vec<Buffer>,
    settings_overlay_buffer: Option<Buffer>,
    /// Shaped terminal rows (integrated PTY view, M26).
    terminal_buffers: Vec<Buffer>,
    /// Sidebar / tab strip / quick-open text lines (M14), shaped as glyphon buffers.
    chrome_overlay_buffers: Vec<Buffer>,
    scale_factor: f32,
}

impl std::fmt::Debug for TextLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextLayer").finish_non_exhaustive()
    }
}

impl TextLayer {
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
            gutter_line_buffers: Vec::new(),
            settings_overlay_buffer: None,
            terminal_buffers: Vec::new(),
            chrome_overlay_buffers: Vec::new(),
            scale_factor: 1.0,
        }
    }

    pub fn set_scale_factor(&mut self, scale: f32) {
        self.scale_factor = scale;
    }

    #[must_use]
    pub fn line_height_px(&self) -> f32 {
        Metrics::new(14.0 * self.scale_factor, 20.0 * self.scale_factor).line_height
    }

    pub fn after_frame(&mut self) {
        self.atlas.trim();
    }

    #[allow(clippy::too_many_arguments)]
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

    fn fill_gutter_buffers(
        &mut self,
        first: usize,
        last: usize,
        total_lines: usize,
        gutter_w: f32,
        metrics: Metrics,
    ) {
        self.gutter_line_buffers.clear();
        let digits = total_lines.max(1).to_string().len().max(1);
        let attrs = Attrs::new().family(Family::Name(BUNDLED_MONO_FAMILY));
        for line_idx in first..last {
            let label = format!("{:>width$}", line_idx + 1, width = digits);
            let mut gbuf = Buffer::new(&mut self.font_system, metrics);
            gbuf.set_size(&mut self.font_system, Some(gutter_w), None);
            gbuf.set_text(&mut self.font_system, &label, &attrs, Shaping::Advanced, None);
            gbuf.shape_until_scroll(&mut self.font_system, false);
            self.gutter_line_buffers.push(gbuf);
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

    fn shape_terminal_buffers(
        &mut self,
        snapshot: &TerminalRenderSnapshot,
        physical_size: PhysicalSize<u32>,
        metrics: Metrics,
        terminal_pane_height_px: f32,
    ) {
        self.terminal_buffers.clear();
        if terminal_pane_height_px <= 0.5 || snapshot.rows.is_empty() {
            return;
        }
        for row in &snapshot.rows {
            for (text, fg, _) in &row.runs {
                let mut buf = Buffer::new(&mut self.font_system, metrics);
                buf.set_size(&mut self.font_system, Some(physical_size.width as f32), None);
                let attrs = Attrs::new()
                    .family(Family::Name(BUNDLED_MONO_FAMILY))
                    .color(Color::rgb(fg[0], fg[1], fg[2]));
                buf.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
                buf.shape_until_scroll(&mut self.font_system, false);
                self.terminal_buffers.push(buf);
            }
        }
    }

    fn prepare_settings_overlay(
        &mut self,
        device: &Device,
        queue: &Queue,
        lines: &[String],
        physical_size: PhysicalSize<u32>,
    ) -> Result<(), RenderError> {
        self.settings_overlay_buffer = None;
        self.terminal_buffers.clear();
        self.chrome_overlay_buffers.clear();
        self.line_buffers.clear();
        self.custom_glyphs_per_line.clear();
        self.gutter_line_buffers.clear();
        self.status_line_buffer = None;
        self.dev_hud_buffer = None;

        let text = lines.join("\n");
        let metrics = Metrics::new(14.0 * self.scale_factor, 20.0 * self.scale_factor);
        let attrs = Attrs::new().family(Family::Name(BUNDLED_MONO_FAMILY));
        let mut buf = Buffer::new(&mut self.font_system, metrics);
        buf.set_size(&mut self.font_system, Some(physical_size.width as f32), None);
        buf.set_text(&mut self.font_system, &text, &attrs, Shaping::Advanced, None);
        buf.shape_until_scroll(&mut self.font_system, false);
        self.settings_overlay_buffer = Some(buf);

        self.viewport.update(
            queue,
            Resolution { width: physical_size.width.max(1), height: physical_size.height.max(1) },
        );

        let w = physical_size.width as i32;
        let h = physical_size.height as i32;
        let areas = vec![TextArea {
            buffer: self.settings_overlay_buffer.as_ref().expect("just set"),
            left: 12.0,
            top: 12.0,
            scale: 1.0,
            bounds: TextBounds { left: 0, top: 0, right: w, bottom: h },
            default_color: Color::rgb(0xE0, 0xE0, 0xE0),
            custom_glyphs: &[],
        }];

        self.text_renderer
            .prepare_with_custom(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                areas,
                &mut self.swash_cache,
                rasterize_cursor,
            )
            .map_err(|e| RenderError::TextPrepare(e.to_string()))
    }

    /// Shapes the document, optional settings overlay, and optional integrated terminal (M26).
    #[allow(clippy::too_many_arguments)]
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
        terminal_pane_height_px: f32,
        terminal_header_height_px: f32,
        terminal_snapshot: Option<&TerminalRenderSnapshot>,
        settings_overlay: Option<&[String]>,
        frame_chrome: Option<&FrameChrome>,
        content_inset_left_px: f32,
        content_inset_top_px: f32,
    ) -> Result<(), RenderError> {
        if let Some(lines) = settings_overlay {
            return self.prepare_settings_overlay(device, queue, lines, physical_size);
        }
        self.settings_overlay_buffer = None;

        let rope = snapshot.rope();
        let total_lines = rope.len_lines();
        let metrics = Metrics::new(14.0 * self.scale_factor, 20.0 * self.scale_factor);
        let line_h = metrics.line_height;

        let status_h = status_bar.map(|s| s.height_px).unwrap_or(0.0);
        let term_h = terminal_pane_height_px.max(0.0);
        let content_px =
            (physical_size.height as f32 - status_h - term_h - content_inset_top_px).max(1.0);
        let clip_bottom = (physical_size.height as f32 - status_h - term_h).round().max(1.0) as i32;

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

        let (gutter_w, _) = compute_gutter_width_px(total_lines, self.scale_factor);
        self.fill_gutter_buffers(first, last, total_lines, gutter_w, metrics);

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

        if let Some(snap) = terminal_snapshot {
            self.shape_terminal_buffers(snap, physical_size, metrics, term_h);
        } else {
            self.terminal_buffers.clear();
        }

        let mut areas = build_layer_text_areas(
            &self.gutter_line_buffers,
            &self.line_buffers,
            &self.custom_glyphs_per_line,
            first,
            gutter_w,
            physical_size,
            scroll,
            line_h,
            clip_bottom,
            content_inset_left_px,
            content_inset_top_px,
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

        if let Some(snap) = terminal_snapshot {
            push_terminal_text_areas(
                &mut areas,
                &self.terminal_buffers,
                snap,
                physical_size,
                self.scale_factor,
                status_h,
                term_h,
                terminal_header_height_px,
                line_h,
                content_inset_left_px,
            );
        }

        push_frame_chrome_text_areas(
            &mut self.font_system,
            &mut self.chrome_overlay_buffers,
            self.scale_factor,
            frame_chrome,
            physical_size,
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
                self.fill_gutter_buffers(first, last, total_lines, gutter_w, metrics);
                if let Some(snap) = terminal_snapshot {
                    self.shape_terminal_buffers(snap, physical_size, metrics, term_h);
                } else {
                    self.terminal_buffers.clear();
                }
                let mut areas2 = build_layer_text_areas(
                    &self.gutter_line_buffers,
                    &self.line_buffers,
                    &self.custom_glyphs_per_line,
                    first,
                    gutter_w,
                    physical_size,
                    scroll,
                    line_h,
                    clip_bottom,
                    content_inset_left_px,
                    content_inset_top_px,
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
                if let Some(snap) = terminal_snapshot {
                    push_terminal_text_areas(
                        &mut areas2,
                        &self.terminal_buffers,
                        snap,
                        physical_size,
                        self.scale_factor,
                        status_h,
                        term_h,
                        terminal_header_height_px,
                        line_h,
                        content_inset_left_px,
                    );
                }
                push_frame_chrome_text_areas(
                    &mut self.font_system,
                    &mut self.chrome_overlay_buffers,
                    self.scale_factor,
                    frame_chrome,
                    physical_size,
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

    #[doc(hidden)]
    #[must_use]
    pub fn test_visible_row_slot_count_for_tests(&self) -> usize {
        self.line_buffers.len()
    }
}
