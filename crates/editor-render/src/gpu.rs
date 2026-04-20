//! `wgpu` surface, device, and swapchain management.

use std::sync::Arc;

use winit::dpi::PhysicalSize;
use winit::window::Window;

use tracing::info;

use crate::error::RenderError;

/// Pick a supported present mode from the surface caps.
///
/// When the display reports **≥ 120 Hz**, prefer [`Mailbox`](wgpu::PresentMode::Mailbox) for
/// lower latency (M12). For 60 Hz / unknown refresh, prefer [`FifoRelaxed`](wgpu::PresentMode::FifoRelaxed)
/// then [`Fifo`](wgpu::PresentMode::Fifo) for stable vsync.
fn choose_present_mode(
    modes: &[wgpu::PresentMode],
    refresh_rate_millihertz: Option<u32>,
) -> wgpu::PresentMode {
    let high_refresh = refresh_rate_millihertz.is_some_and(|hz| hz >= 120_000);

    let preference: &[wgpu::PresentMode] = if high_refresh {
        &[
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::FifoRelaxed,
            wgpu::PresentMode::Fifo,
            wgpu::PresentMode::Immediate,
        ]
    } else {
        &[
            wgpu::PresentMode::FifoRelaxed,
            wgpu::PresentMode::Fifo,
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::Immediate,
        ]
    };

    for &mode in preference {
        if modes.contains(&mode) {
            return mode;
        }
    }
    modes.first().copied().unwrap_or(wgpu::PresentMode::Fifo)
}

fn request_adapter_for_surface(
    instance: &wgpu::Instance,
    surface: &wgpu::Surface<'_>,
) -> Result<wgpu::Adapter, RenderError> {
    let attempts: [(wgpu::PowerPreference, bool); 4] = [
        (wgpu::PowerPreference::HighPerformance, false),
        (wgpu::PowerPreference::HighPerformance, true),
        (wgpu::PowerPreference::LowPower, false),
        (wgpu::PowerPreference::LowPower, true),
    ];

    for (power_preference, force_fallback_adapter) in attempts {
        match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference,
            compatible_surface: Some(surface),
            force_fallback_adapter,
        })) {
            Ok(adapter) => {
                let adapter_info = adapter.get_info();
                info!(
                    ?power_preference,
                    force_fallback_adapter,
                    backend = ?adapter_info.backend,
                    adapter = %adapter_info.name,
                    "wgpu: selected adapter"
                );
                return Ok(adapter);
            }
            Err(e) => tracing::debug!(
                ?power_preference,
                force_fallback_adapter,
                error = %e,
                "wgpu: adapter request failed, retrying"
            ),
        }
    }

    Err(RenderError::SurfaceCreate(
        "no compatible GPU adapter found (tried high-performance, low-power, and fallback)".into(),
    ))
}

/// Owns `wgpu` instance, surface, device, queue, and swapchain configuration.
pub struct GpuContext {
    pub(crate) surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    format: wgpu::TextureFormat,
}

impl std::fmt::Debug for GpuContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuContext")
            .field("config", &self.config)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}

impl GpuContext {
    /// Creates a GPU context bound to a native window (`winit` 0.30+).
    ///
    /// Uses `pollster` only at this boundary; library callers must not block
    /// the real-time frame loop on async `wgpu` APIs.
    ///
    /// Takes ownership of the `Arc` because the surface may retain a handle to
    /// the window for its lifetime; passing by value matches the intended
    /// one-owner-per-init call site.
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(window: Arc<Window>) -> Result<Self, RenderError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| RenderError::SurfaceCreate(e.to_string()))?;

        let adapter = request_adapter_for_surface(&instance, &surface)?;

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
        let refresh = window.current_monitor().and_then(|m| m.refresh_rate_millihertz());
        config.present_mode = choose_present_mode(&caps.present_modes, refresh);
        if let Some(hz) = refresh {
            info!(
                refresh_hz = hz as f64 / 1000.0,
                present_mode = ?config.present_mode,
                "wgpu: initial present mode"
            );
        }

        let device_desc = wgpu::DeviceDescriptor {
            label: Some("ide-gpu"),
            required_limits: adapter.limits().clone(),
            ..Default::default()
        };

        let (device, queue) = pollster::block_on(adapter.request_device(&device_desc))?;

        let format = config.format;
        surface.configure(&device, &config);

        Ok(Self { surface, adapter, device, queue, config, format })
    }

    /// Re-evaluate present mode when the window may have moved to a display with a different
    /// refresh rate (multi-monitor).
    pub fn sync_present_mode_for_window(&mut self, window: &Window) {
        let refresh = window.current_monitor().and_then(|m| m.refresh_rate_millihertz());
        let caps = self.surface.get_capabilities(&self.adapter);
        let mode = choose_present_mode(&caps.present_modes, refresh);
        if mode != self.config.present_mode {
            info!(?mode, ?refresh, "wgpu: present mode updated for current display");
            self.config.present_mode = mode;
            self.surface.configure(&self.device, &self.config);
        }
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
        self.render_with_pass(color, |_pass| Ok(()))
    }

    /// Clears to `color`, runs `draw` inside the render pass, then presents.
    pub fn render_with_pass<F>(&mut self, color: wgpu::Color, draw: F) -> Result<(), RenderError>
    where
        F: FnOnce(&mut wgpu::RenderPass<'_>) -> Result<(), RenderError>,
    {
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

        let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame-pass") });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
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
            draw(&mut pass)?;
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

    /// Swapchain width / height in pixels.
    #[must_use]
    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize::new(self.config.width, self.config.height)
    }

    /// GPU device.
    #[must_use]
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Submission queue.
    #[must_use]
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
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
