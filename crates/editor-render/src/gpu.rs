//! `wgpu` surface, device, and swapchain management.

use std::sync::Arc;

use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::error::RenderError;

/// Owns `wgpu` instance, surface, device, queue, and swapchain configuration.
#[derive(Debug)]
pub struct GpuContext {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    format: wgpu::TextureFormat,
}

impl GpuContext {
    /// Creates a GPU context bound to a native window (`winit` 0.30+).
    ///
    /// Uses `pollster` only at this boundary; library callers must not block
    /// the real-time frame loop on async `wgpu` APIs.
    pub fn new(window: &Arc<Window>) -> Result<Self, RenderError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| RenderError::SurfaceCreate(e.to_string()))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))?;

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| {
                RenderError::SurfaceCreate("surface not compatible with adapter".into())
            })?;

        // Prefer sRGB formats when available.
        let caps = surface.get_capabilities(&adapter);
        if let Some(srgb) = caps.formats.iter().copied().find(|f| f.is_srgb()) {
            config.format = srgb;
        }
        config.present_mode = wgpu::PresentMode::Fifo;

        let device_desc = wgpu::DeviceDescriptor {
            label: Some("ide-gpu"),
            required_limits: adapter.limits().clone(),
            ..Default::default()
        };

        let (device, queue) = pollster::block_on(adapter.request_device(&device_desc))?;

        let format = config.format;
        surface.configure(&device, &config);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            format,
        })
    }

    /// Reconfigures the swapchain after a resize or DPI change.
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Clears the surface to `color` and presents.
    pub fn render_clear(&mut self, color: wgpu::Color) -> Result<(), RenderError> {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                return Err(RenderError::SurfaceTexture("surface lost"));
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(RenderError::SurfaceTexture("surface validation error"));
            }
        };

        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clear-pass"),
            });

        {
            let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
        Ok(())
    }

    /// Preferred surface format chosen for this context.
    #[must_use]
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

/// Headless smoke test: pick an adapter and device without creating a surface.
///
/// Intended for CI and `editor-app --dry-run` when no window is available.
pub fn dry_run_headless() -> Result<(), RenderError> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: true,
    }))?;

    let device_desc = wgpu::DeviceDescriptor {
        label: Some("ide-dry-run"),
        required_limits: adapter.limits().clone(),
        ..Default::default()
    };

    let (_device, _queue) = pollster::block_on(adapter.request_device(&device_desc))?;

    Ok(())
}
