//! `editor-app` — binary shell for the IDE project.
//!
//! M01 opens a `winit` window and clears the framebuffer via `wgpu` each frame.
//!
//! See `docs/ARCHITECTURE.md` for wiring and `docs/MISSIONS.md` for the plan.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use std::sync::Arc;

use anyhow::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

/// Crate / app version from `Cargo.toml`.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Dark background (`#1e1e1e`).
const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.118,
    g: 0.118,
    b: 0.118,
    a: 1.0,
};

fn main() -> Result<()> {
    init_tracing();

    let mut args = std::env::args();
    args.next();
    if args.any(|a| a == "--dry-run") {
        info!(version = VERSION, "ide: dry-run (headless GPU init)");
        editor_render::dry_run_headless()?;
        info!("ide: dry-run OK");
        return Ok(());
    }

    info!(version = VERSION, "ide: starting windowed mode");
    info!("linked: {}", editor_core::banner());
    info!("linked: {}", editor_render::banner());
    info!("linked: {}", editor_input::banner());
    info!("linked: {}", editor_io::banner());
    info!("linked: {}", editor_ui::banner());

    let event_loop = EventLoop::new()?;
    let mut app = App {
        window: None,
        gpu: None,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<editor_render::GpuContext>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("IDE"))
                .expect("create window"),
        );
        let gpu = editor_render::GpuContext::new(&window).expect("init GPU");
        window.request_redraw();
        self.window = Some(window);
        self.gpu = Some(gpu);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(size);
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = self.gpu.as_mut() {
                    if let Err(e) = gpu.render_clear(CLEAR) {
                        tracing::warn!(error = %e, "render frame");
                    }
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,editor_app=info,editor_render=info,wgpu=warn"));

    if cfg!(debug_assertions) {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .with_level(true)
            .pretty()
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .with_level(true)
            .compact()
            .init();
    }

    info!("editor-app v{VERSION} starting");
}
