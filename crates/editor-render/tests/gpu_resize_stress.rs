//! M08 §5 — Tight loop of swapchain resizes; must not panic or trip wgpu validation (local/CI with GPU).
//!
//! Skip gracefully when the runner has no usable adapter/window (common on headless Linux CI).

#![allow(clippy::print_stderr)] // Skip reasons are printed for local debugging when GPU/window absent.

use std::sync::Arc;

use editor_core::{ScrollOffset, TextBuffer, TextBufferSnapshot};
use editor_render::{EditorRenderer, FrameInput};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

fn event_loop_for_test() -> Result<EventLoop<()>, winit::error::EventLoopError> {
    let mut builder = EventLoop::builder();
    // `cargo test` runs tests on worker threads; winit requires allow-listing off-main on some OSes.
    #[cfg(windows)]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        let _ = builder.with_any_thread(true);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        let _ = builder.with_any_thread(true);
    }
    builder.build()
}

fn paint_resize_frame(
    renderer: &mut EditorRenderer,
    snap: &TextBufferSnapshot,
    sz: PhysicalSize<u32>,
    scale: f32,
) {
    renderer.resize(sz);
    let input = FrameInput {
        buffer: snap,
        scroll: ScrollOffset { y_px: 0.0 },
        clear_color: wgpu::Color::BLACK,
        cursor_byte: 0,
        cursor_blink_on: false,
        physical_size: sz,
        scale_factor: scale,
        status: None,
        dev_hud_line: None,
        selection_byte_range: None,
        diff: None,
        terminal_pane_height_px: 0.0,
        terminal_header_height_px: 0.0,
        terminal_snapshot: None,
        settings_overlay_lines: None,
        frame_chrome: None,
        content_inset_left_px: 0.0,
        content_inset_top_px: 0.0,
    };
    renderer.render_frame(&input).expect("render_frame");
}

struct RapidResizeStress {
    done: bool,
}

impl ApplicationHandler for RapidResizeStress {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.done {
            return;
        }
        self.done = true;

        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title("gpu_resize_stress")
                .with_visible(false)
                .with_inner_size(PhysicalSize::new(400, 300)),
        ) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("gpu_resize_stress: skip — window creation failed ({e})");
                event_loop.exit();
                return;
            }
        };

        let mut renderer = match EditorRenderer::new(window) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("gpu_resize_stress: skip — GPU init failed ({e})");
                event_loop.exit();
                return;
            }
        };

        for i in 0_u32..100 {
            let w = 64 + (i.wrapping_mul(67) % 960);
            let h = 48 + (i.wrapping_mul(91) % 720);
            renderer.resize(PhysicalSize::new(w.max(1), h.max(1)));
        }

        event_loop.exit();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let WindowEvent::CloseRequested = event {
            event_loop.exit();
        }
    }
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "winit EventLoop is main-thread only on macOS; run this test single-threaded locally if needed"
)]
fn rapid_resize_100_iterations_no_panic() {
    let event_loop = match event_loop_for_test() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("gpu_resize_stress: skip — EventLoop builder failed ({e})");
            return;
        }
    };
    let mut app = RapidResizeStress { done: false };
    let _ = event_loop.run_app(&mut app);
}

struct ResizeThenPaintScratchStable {
    done: bool,
}

impl ApplicationHandler for ResizeThenPaintScratchStable {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.done {
            return;
        }
        self.done = true;

        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title("gpu_resize_scratch_stable")
                .with_visible(false)
                .with_inner_size(PhysicalSize::new(480, 360)),
        ) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("gpu_resize_scratch_stable: skip — window creation failed ({e})");
                event_loop.exit();
                return;
            }
        };

        let mut renderer = match EditorRenderer::new(window.clone()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("gpu_resize_scratch_stable: skip — GPU init failed ({e})");
                event_loop.exit();
                return;
            }
        };

        let buf = TextBuffer::new();
        let snap = buf.snapshot();
        let scale = window.scale_factor() as f32;

        paint_resize_frame(&mut renderer, &snap, PhysicalSize::new(480, 360), scale);
        let slots = renderer.test_visible_row_slot_count();

        for i in 0_u32..80 {
            let w = 64 + (i.wrapping_mul(67) % 1400);
            let h = 48 + (i.wrapping_mul(91) % 1050);
            paint_resize_frame(&mut renderer, &snap, PhysicalSize::new(w.max(1), h.max(1)), scale);
        }

        assert_eq!(
            renderer.test_visible_row_slot_count(),
            slots,
            "visible row scratch should not grow after warm-up (M12)"
        );

        event_loop.exit();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let WindowEvent::CloseRequested = event {
            event_loop.exit();
        }
    }
}

#[test]
#[cfg_attr(
    target_os = "macos",
    ignore = "winit EventLoop is main-thread only on macOS; run this test single-threaded locally if needed"
)]
fn resize_with_paint_does_not_grow_visible_row_scratch() {
    let event_loop = match event_loop_for_test() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("gpu_resize_scratch_stable: skip — EventLoop builder failed ({e})");
            return;
        }
    };
    let mut app = ResizeThenPaintScratchStable { done: false };
    let _ = event_loop.run_app(&mut app);
}
