//! Composes [`GpuContext`](crate::gpu::GpuContext) + [`TextLayer`](crate::text_layer::TextLayer) for the editor window.

use std::sync::Arc;
use std::time::{Duration, Instant};

use editor_core::{ScrollOffset, TextBufferSnapshot};
use editor_diff::DiffPaint;
use editor_ui::{StatusBarInfo, StatusBarLayout};
use wgpu::Color;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::diff_layout;
use crate::error::RenderError;
use crate::gpu::GpuContext;
use crate::selection_layout;
use crate::solid_quads::SolidQuadLayer;
use crate::text_layer::TextLayer;

/// Per-frame inputs for the compositor.
#[derive(Debug)]
pub struct FrameInput<'a> {
    /// Document snapshot to draw.
    pub buffer: &'a TextBufferSnapshot,
    /// Vertical scroll in physical pixels.
    pub scroll: ScrollOffset,
    /// Clear color before text.
    pub clear_color: Color,
    /// Caret position in UTF-8 bytes.
    pub cursor_byte: usize,
    /// Whether the caret should be visible (blink phase).
    pub cursor_blink_on: bool,
    /// Viewport in physical pixels.
    pub physical_size: PhysicalSize<u32>,
    /// Window content scale factor (DPI).
    pub scale_factor: f32,
    /// Bottom status bar (file path, Ln/Col, encoding, line ending). `None` hides it.
    pub status: Option<StatusBarInfo>,
    /// Top-right dev overlay line (e.g. frame percentiles). `None` skips overlay.
    pub dev_hud_line: Option<String>,
    /// UTF-8 byte range `[lo, hi)` to highlight (non-empty selections). `None` skips quads.
    pub selection_byte_range: Option<(usize, usize)>,
    /// Inline diff tint quads (M17/M18). `None` skips diff overlays.
    pub diff: Option<DiffPaint<'a>>,
    /// Height reserved at the bottom for the integrated terminal (M26). `0.0` hides the pane.
    pub terminal_pane_height_px: f32,
    /// Height of the terminal pane's header strip (drawn by `editor-ui::terminal_header`).
    /// Pulls the PTY rows down by this many physical pixels so the label + close button
    /// sit above the shell output. `0.0` disables the offset (backwards compatible).
    pub terminal_header_height_px: f32,
    /// Pre-shaped terminal grid (PTY + alacritty). `None` when the pane is hidden or not ready.
    pub terminal_snapshot: Option<editor_terminal::TerminalRenderSnapshot>,
    /// When set, replaces the document view with a full-window settings overlay (M28).
    pub settings_overlay_lines: Option<&'a [String]>,
    /// Sidebar / tab strip / quick-open quads + text (M14). Drawn above the editor body text.
    pub frame_chrome: Option<&'a editor_ui::FrameChrome>,
    /// M14: shift document text + selection insets (physical px). Zero when chrome is off.
    pub content_inset_left_px: f32,
    pub content_inset_top_px: f32,
    /// Right inset (physical px) for panels on the right side (e.g. agent panel).
    /// Text and terminal rendering clip to `window_width - content_inset_right_px`.
    pub content_inset_right_px: f32,
    /// When > 0, terminal content renders starting at this x position (physical px).
    /// Set to the agent panel's left edge to keep the terminal inside the right panel.
    pub terminal_left_px: f32,
    /// When > 0, terminal content clips to this x position (physical px).
    /// Set to the window width to let terminal fill the full right panel width.
    pub terminal_right_px: f32,
    /// Source language for syntax highlighting (M15). [`editor_syntax::Language::Plain`]
    /// preserves the pre-highlight path (single attrs per line) with zero overhead.
    pub language: editor_syntax::Language,
}

/// CPU/GPU split for one presented frame (M07).
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameTimings {
    pub prepare: Duration,
    pub gpu: Duration,
    pub total: Duration,
}

/// Window GPU context plus glyphon text pipeline.
pub struct EditorRenderer {
    gpu: GpuContext,
    text: TextLayer,
    solid: SolidQuadLayer,
    /// Reused allocation for selection quads (avoid per-frame `Vec` churn).
    selection_rect_scratch: Vec<(f32, f32, f32, f32, [f32; 4])>,
}

impl std::fmt::Debug for EditorRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorRenderer").finish_non_exhaustive()
    }
}

impl EditorRenderer {
    /// Initializes wgpu + glyphon for `window`.
    pub fn new(window: Arc<Window>) -> Result<Self, RenderError> {
        let gpu = GpuContext::new(window)?;
        let format = gpu.surface_format();
        let text = TextLayer::new(gpu.device(), gpu.queue(), format);
        let solid = SolidQuadLayer::new(gpu.device(), format);
        Ok(Self { gpu, text, solid, selection_rect_scratch: Vec::new() })
    }

    /// Swapchain resize (window size / DPI).
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.gpu.resize(new_size);
    }

    /// Refresh [`wgpu::PresentMode`] if the window moved to a monitor with a different refresh rate.
    pub fn sync_present_mode(&mut self, window: &Window) {
        self.gpu.sync_present_mode_for_window(window);
    }

    /// Window content scale factor from winit (optional; also set per frame via [`FrameInput`]).
    pub fn set_scale_factor(&mut self, scale: f32) {
        self.text.set_scale_factor(scale);
    }

    /// Clear, shape text, draw, present. Returns per-phase timings when work was submitted.
    #[tracing::instrument(skip(self, input), level = "debug")]
    pub fn render_frame(&mut self, input: &FrameInput<'_>) -> Result<FrameTimings, RenderError> {
        let frame_start = Instant::now();
        self.text.set_scale_factor(input.scale_factor);
        let EditorRenderer { gpu, text, solid, selection_rect_scratch } = self;
        let status_bar =
            input.status.as_ref().map(|s| StatusBarLayout::from_info(s, input.scale_factor));
        let dev_hud = input.dev_hud_line.as_deref();
        let status_h = status_bar.as_ref().map(|s| s.height_px).unwrap_or(0.0);
        let rope = input.buffer.rope();
        let total_lines = rope.len_lines();
        let (gutter_w, char_w) =
            crate::text_layer::compute_gutter_width_px(total_lines, input.scale_factor);
        let line_h = text.line_height_px();
        selection_rect_scratch.clear();
        let term_h = input.terminal_pane_height_px.max(0.0);
        if let Some((a, b)) = input.selection_byte_range {
            selection_layout::selection_rects_pixels_into(
                selection_rect_scratch,
                rope,
                a,
                b,
                input.scroll,
                input.physical_size,
                status_h,
                term_h,
                line_h,
                gutter_w,
                char_w,
                input.content_inset_left_px,
                input.content_inset_top_px,
            );
        }
        if let Some(diff) = input.diff {
            diff_layout::inline_diff_quads_into(
                selection_rect_scratch,
                rope,
                diff.lines,
                input.scroll,
                input.physical_size,
                status_h,
                term_h,
                line_h,
                gutter_w,
                char_w,
                input.content_inset_left_px,
                input.content_inset_top_px,
            );
        }
        if let Some(fc) = input.frame_chrome {
            for q in &fc.quads {
                let l = q.left;
                let t = q.top;
                let r = q.left + q.width;
                let b = q.top + q.height;
                selection_rect_scratch.push((l, t, r, b, q.rgba));
            }
        }
        let t_prep = Instant::now();
        solid.prepare(
            gpu.device(),
            gpu.queue(),
            input.physical_size.width,
            input.physical_size.height,
            selection_rect_scratch.as_slice(),
        );
        text.prepare(
            gpu.device(),
            gpu.queue(),
            input.buffer,
            input.scroll,
            input.cursor_byte,
            input.physical_size,
            input.cursor_blink_on,
            status_bar.as_ref(),
            dev_hud,
            term_h,
            input.terminal_header_height_px,
            input.terminal_snapshot.as_ref(),
            input.settings_overlay_lines,
            input.frame_chrome,
            input.content_inset_left_px,
            input.content_inset_top_px,
            input.content_inset_right_px,
            input.terminal_left_px,
            input.terminal_right_px,
            input.language,
        )?;
        let prepare = t_prep.elapsed();

        let t_gpu = Instant::now();
        let surface_texture = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                text.after_frame();
                return Ok(FrameTimings {
                    prepare,
                    gpu: Duration::ZERO,
                    total: frame_start.elapsed(),
                });
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                gpu.surface.configure(gpu.device(), &gpu.config);
                text.after_frame();
                return Ok(FrameTimings {
                    prepare,
                    gpu: Duration::ZERO,
                    total: frame_start.elapsed(),
                });
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                return Err(RenderError::SurfaceTexture("surface lost"));
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(RenderError::SurfaceTexture("surface validation error"));
            }
        };

        let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("editor-frame"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(input.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            solid.render(&mut pass);
            text.render(&mut pass)?;
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
        let gpu_elapsed = t_gpu.elapsed();
        text.after_frame();
        Ok(FrameTimings { prepare, gpu: gpu_elapsed, total: frame_start.elapsed() })
    }

    /// Default line height in physical pixels (matches bundled metrics × scale).
    #[must_use]
    pub fn line_height_px(&self) -> f32 {
        self.text.line_height_px()
    }

    /// Row scratch capacity after [`Self::render_frame`] pre-warm (integration tests; M12).
    #[doc(hidden)]
    #[must_use]
    pub fn test_visible_row_slot_count(&self) -> usize {
        self.text.test_visible_row_slot_count_for_tests()
    }
}
