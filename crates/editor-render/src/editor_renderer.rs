//! Composes [`GpuContext`](crate::gpu::GpuContext) + [`TextLayer`](crate::text_layer::TextLayer) for the editor window.

use std::sync::Arc;
use std::time::{Duration, Instant};

use editor_core::{ScrollOffset, TextBufferSnapshot};
use editor_ui::{StatusBarInfo, StatusBarLayout};
use wgpu::Color;
use winit::dpi::PhysicalSize;
use winit::window::Window;

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
        Ok(Self { gpu, text, solid })
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
        let EditorRenderer { gpu, text, solid } = self;
        let status_bar =
            input.status.as_ref().map(|s| StatusBarLayout::from_info(s, input.scale_factor));
        let dev_hud = input.dev_hud_line.as_deref();
        let status_h = status_bar.as_ref().map(|s| s.height_px).unwrap_or(0.0);
        let rope = input.buffer.rope();
        let total_lines = rope.len_lines();
        let (gutter_w, char_w) =
            crate::text_layer::compute_gutter_width_px(total_lines, input.scale_factor);
        let line_h = text.line_height_px();
        let rects = if let Some((a, b)) = input.selection_byte_range {
            selection_layout::selection_rects_pixels(
                rope,
                a,
                b,
                input.scroll,
                input.physical_size,
                status_h,
                line_h,
                gutter_w,
                char_w,
            )
        } else {
            Vec::new()
        };
        let t_prep = Instant::now();
        solid.prepare(
            gpu.device(),
            gpu.queue(),
            input.physical_size.width,
            input.physical_size.height,
            &rects,
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
}
