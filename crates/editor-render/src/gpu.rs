//! `wgpu` surface, device, and swapchain management.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use winit::dpi::PhysicalSize;
use winit::window::Window;

use tracing::{info, warn};

use crate::backend;
use crate::error::RenderError;

/// Try full adapter limits first, then wgpu downlevel presets merged with adapter resolution /
/// alignment so swapchain-sized surfaces still work on older drivers (D3D11 / GLES / quirky Vulkan).
fn request_device_with_limit_fallbacks(
    adapter: &wgpu::Adapter,
    label: &'static str,
) -> Result<(wgpu::Device, wgpu::Queue), RenderError> {
    const STRATEGY: [&str; 3] = ["adapter_max", "downlevel_defaults", "downlevel_webgl2"];

    let mut last_err: Option<wgpu::RequestDeviceError> = None;
    for (i, name) in STRATEGY.iter().enumerate() {
        let required_limits = match i {
            0 => adapter.limits(),
            1 => {
                let c = adapter.limits();
                wgpu::Limits::downlevel_defaults().using_resolution(c.clone()).using_alignment(c)
            }
            2 => {
                let c = adapter.limits();
                wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(c.clone())
                    .using_alignment(c)
            }
            _ => unreachable!(),
        };

        let desc = wgpu::DeviceDescriptor {
            label: Some(label),
            required_features: wgpu::Features::empty(),
            required_limits,
            ..Default::default()
        };

        match pollster::block_on(adapter.request_device(&desc)) {
            Ok(pair) => {
                if i > 0 {
                    info!(
                        strategy = *name,
                        "wgpu: device created using conservative limits (portability fallback)"
                    );
                }
                return Ok(pair);
            }
            Err(e) => {
                warn!(strategy = *name, error = %e, "wgpu: device request failed");
                last_err = Some(e);
            }
        }
    }

    Err(match last_err {
        Some(e) => RenderError::RequestDevice(e),
        None => {
            RenderError::SurfaceCreate("wgpu: device request produced no error (internal)".into())
        }
    })
}

/// Displays reporting at least this rate (millihertz) use an ~8 ms frame budget (120 Hz); others use ~16 ms (60 Hz).
const HIGH_REFRESH_MILLIHZ: u32 = 120_000;

#[inline]
fn refresh_rate_is_high(refresh_rate_millihertz: Option<u32>) -> bool {
    refresh_rate_millihertz.is_some_and(|hz| hz >= HIGH_REFRESH_MILLIHZ)
}

/// Optional override from `IDE_PRESENT_MODE` (`immediate`, `fifo`, `fifo_relaxed`, `mailbox`).
fn present_mode_from_env() -> Option<wgpu::PresentMode> {
    let s = std::env::var("IDE_PRESENT_MODE").ok()?;
    Some(match s.trim().to_ascii_lowercase().as_str() {
        "immediate" => wgpu::PresentMode::Immediate,
        "fifo" => wgpu::PresentMode::Fifo,
        "fifo_relaxed" | "fifo-relaxed" | "fiforelaxed" => wgpu::PresentMode::FifoRelaxed,
        "mailbox" => wgpu::PresentMode::Mailbox,
        _ => return None,
    })
}

/// Adapter request order. `IDE_POWER_PREFERENCE=low` prefers integrated / low-power first.
fn adapter_attempts() -> [(wgpu::PowerPreference, bool); 4] {
    match std::env::var("IDE_POWER_PREFERENCE").ok().as_deref() {
        Some("low") | Some("low_power") => [
            (wgpu::PowerPreference::LowPower, false),
            (wgpu::PowerPreference::LowPower, true),
            (wgpu::PowerPreference::HighPerformance, false),
            (wgpu::PowerPreference::HighPerformance, true),
        ],
        _ => [
            (wgpu::PowerPreference::HighPerformance, false),
            (wgpu::PowerPreference::HighPerformance, true),
            (wgpu::PowerPreference::LowPower, false),
            (wgpu::PowerPreference::LowPower, true),
        ],
    }
}

/// Pick a supported present mode from the surface caps.
///
/// When the display reports **≥ 120 Hz**, prefer [`Mailbox`](wgpu::PresentMode::Mailbox) for
/// lower latency (M12). For 60 Hz / unknown refresh, prefer [`FifoRelaxed`](wgpu::PresentMode::FifoRelaxed)
/// then [`Fifo`](wgpu::PresentMode::Fifo) for stable vsync.
fn choose_present_mode(
    modes: &[wgpu::PresentMode],
    refresh_rate_millihertz: Option<u32>,
    env_override: Option<wgpu::PresentMode>,
) -> wgpu::PresentMode {
    if let Some(want) = env_override {
        if modes.contains(&want) {
            return want;
        }
        warn!(
            ?want,
            supported = ?modes,
            "IDE_PRESENT_MODE not supported; using automatic choice"
        );
    }

    let high_refresh = refresh_rate_is_high(refresh_rate_millihertz);

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
    for (power_preference, force_fallback_adapter) in adapter_attempts() {
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
                    vendor = adapter_info.vendor,
                    device_id = format_args!("0x{:x}", adapter_info.device),
                    device_type = ?adapter_info.device_type,
                    driver = %adapter_info.driver,
                    driver_info = %adapter_info.driver_info,
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

fn request_adapter_headless(instance: &wgpu::Instance) -> Result<wgpu::Adapter, RenderError> {
    for (power_preference, force_fallback_adapter) in adapter_attempts() {
        match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference,
            compatible_surface: None,
            force_fallback_adapter,
        })) {
            Ok(adapter) => {
                let adapter_info = adapter.get_info();
                info!(
                    ?power_preference,
                    force_fallback_adapter,
                    backend = ?adapter_info.backend,
                    adapter = %adapter_info.name,
                    vendor = adapter_info.vendor,
                    device_id = format_args!("0x{:x}", adapter_info.device),
                    device_type = ?adapter_info.device_type,
                    driver = %adapter_info.driver,
                    driver_info = %adapter_info.driver_info,
                    "wgpu: dry-run selected adapter"
                );
                return Ok(adapter);
            }
            Err(e) => tracing::debug!(
                ?power_preference,
                force_fallback_adapter,
                error = %e,
                "wgpu: headless adapter request failed, retrying"
            ),
        }
    }

    Err(RenderError::NoAdapter)
}

/// Owns `wgpu` instance, surface, device, queue, and swapchain configuration.
pub struct GpuContext {
    pub(crate) surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    format: wgpu::TextureFormat,
    high_refresh_display: bool,
    env_present_override: Option<wgpu::PresentMode>,
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
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = backend::instance_backends();
        let instance = wgpu::Instance::new(desc);

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
        let high_refresh_display = refresh_rate_is_high(refresh);
        let env_present_override = present_mode_from_env();
        config.present_mode =
            choose_present_mode(&caps.present_modes, refresh, env_present_override);
        if let Some(hz) = refresh {
            info!(
                refresh_hz = hz as f64 / 1000.0,
                present_mode = ?config.present_mode,
                "wgpu: initial present mode"
            );
        }

        let (device, queue) = request_device_with_limit_fallbacks(&adapter, "ide-gpu")?;

        let format = config.format;
        surface.configure(&device, &config);

        Ok(Self {
            surface,
            adapter,
            device,
            queue,
            config,
            format,
            high_refresh_display,
            env_present_override,
        })
    }

    /// `true` when the current monitor reports ≥120 Hz (used for frame budget hints).
    #[must_use]
    pub fn display_refresh_is_high(&self) -> bool {
        self.high_refresh_display
    }

    /// Re-evaluate present mode when the window may have moved to a display with a different
    /// refresh rate (multi-monitor).
    pub fn sync_present_mode_for_window(&mut self, window: &Window) {
        let refresh = window.current_monitor().and_then(|m| m.refresh_rate_millihertz());
        self.high_refresh_display = refresh_rate_is_high(refresh);
        let caps = self.surface.get_capabilities(&self.adapter);
        let mode = choose_present_mode(&caps.present_modes, refresh, self.env_present_override);
        if mode != self.config.present_mode {
            info!(?mode, ?refresh, "wgpu: present mode updated for current display");
            self.config.present_mode = mode;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Reconfigures the swapchain after a resize or DPI change.
    ///
    /// The presentation image is always **window-sized**; the driver reallocates swapchain storage
    /// on `configure`. Application-owned GPU resources that are **not** the swapchain (solid-quad
    /// vertex buffer, glyph atlas budget, pre-sized text row scratch in [`crate::text_layer::TextLayer`])
    /// stay allocated across resize (M12).
    ///
    /// Mixed-DPI: some platforms emit `ScaleFactorChanged` when crossing monitors; others only
    /// update physical size on the next `Resized` — always read [`winit::window::Window::inner_size`]
    /// for the current drawable extent.
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Acquire a swapchain texture, reconfiguring after outdated/lost surfaces (limited retries).
    pub(crate) fn acquire_present_surface_texture(
        &mut self,
    ) -> Result<Option<wgpu::SurfaceTexture>, RenderError> {
        for attempt in 0..3 {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t)
                | wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                    return Ok(Some(t));
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    return Ok(None);
                }
                wgpu::CurrentSurfaceTexture::Outdated => {
                    self.surface.configure(&self.device, &self.config);
                    if attempt == 2 {
                        return Ok(None);
                    }
                }
                wgpu::CurrentSurfaceTexture::Lost => {
                    // Sleep/wake and some drivers can report `Lost`; brief pause before reconfigure.
                    thread::sleep(Duration::from_millis(50));
                    self.surface.configure(&self.device, &self.config);
                    if attempt == 2 {
                        return Err(RenderError::SurfaceTexture("surface lost"));
                    }
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(RenderError::SurfaceTexture("surface validation error"));
                }
            }
        }
        Ok(None)
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
        let Some(surface_texture) = self.acquire_present_surface_texture()? else {
            return Ok(());
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

    /// Active swapchain configuration (extent, format, present mode).
    #[must_use]
    pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
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
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backend::instance_backends();
    let instance = wgpu::Instance::new(desc);

    let adapter = request_adapter_headless(&instance)?;

    let (_device, _queue) = request_device_with_limit_fallbacks(&adapter, "ide-dry-run")?;

    Ok(())
}

static_assertions::assert_impl_all!(GpuContext: Send, Sync);

#[cfg(test)]
mod present_mode_tests {
    use super::choose_present_mode;
    use wgpu::PresentMode;

    #[test]
    fn high_refresh_prefers_mailbox_when_supported() {
        let modes = [PresentMode::Fifo, PresentMode::Mailbox, PresentMode::Immediate];
        assert_eq!(choose_present_mode(&modes, Some(144_000), None), PresentMode::Mailbox);
    }

    #[test]
    fn low_refresh_prefers_fifo_relaxed_when_supported() {
        let modes = [PresentMode::Fifo, PresentMode::FifoRelaxed, PresentMode::Mailbox];
        assert_eq!(choose_present_mode(&modes, Some(60_000), None), PresentMode::FifoRelaxed);
    }

    #[test]
    fn unknown_refresh_prefers_fifo_relaxed_when_supported() {
        let modes = [PresentMode::Immediate, PresentMode::FifoRelaxed];
        assert_eq!(choose_present_mode(&modes, None, None), PresentMode::FifoRelaxed);
    }

    #[test]
    fn high_refresh_falls_back_to_fifo_when_mailbox_unavailable() {
        let modes = [PresentMode::Immediate, PresentMode::Fifo];
        assert_eq!(choose_present_mode(&modes, Some(144_000), None), PresentMode::Fifo);
    }
}
