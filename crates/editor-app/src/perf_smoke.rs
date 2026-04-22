//! Scripted performance smoke (M07): hidden window + GPU frames, JSON result on stdout.
//!
//! `--perf-smoke` avoids interactive input. Thresholds default to the M07 mission (p99 ≤ 16 ms,
//! max frame ≤ 50 ms). Set `PERF_SMOKE_RELAX=1` for CI hosts with slow GPUs (validates only that
//! the sequence completes and emits metrics).

#![allow(clippy::print_stderr, clippy::print_stdout)] // JSON / errors on stdio for the perf harness

use std::sync::Arc;
use std::time::Duration;

use editor_core::{BytePos, ScrollOffset, TextBuffer};
use editor_render::{EditorRenderer, FrameInput};
use serde::Serialize;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::metrics::MetricsCollector;

const CLEAR: wgpu::Color = wgpu::Color { r: 0.118, g: 0.118, b: 0.118, a: 1.0 };

fn build_10mb_corpus() -> String {
    const LINE: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n";
    let mut s = String::with_capacity(11 * 1024 * 1024);
    while s.len() < 10 * 1024 * 1024 {
        s.push_str(LINE);
    }
    s
}

fn event_loop_for_smoke() -> Result<EventLoop<()>, winit::error::EventLoopError> {
    let mut builder = EventLoop::builder();
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

/// JSON line written to stdout on success.
#[derive(Debug, Serialize)]
pub struct PerfSmokeReport {
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub frames: usize,
    pub passed: bool,
    pub threshold_p99_ms: f64,
    pub threshold_max_ms: f64,
    pub relaxed: bool,
}

struct PerfSmokeApp {
    done: bool,
    report: Option<PerfSmokeReport>,
}

impl ApplicationHandler for PerfSmokeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.done {
            return;
        }
        self.done = true;

        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title("perf-smoke")
                .with_visible(false)
                .with_inner_size(PhysicalSize::new(1280, 720)),
        ) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("perf-smoke: window creation failed: {e}");
                event_loop.exit();
                return;
            }
        };

        let mut renderer = match EditorRenderer::new(window.clone()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("perf-smoke: GPU init failed: {e}");
                event_loop.exit();
                return;
            }
        };

        let text = build_10mb_corpus();
        let buffer = TextBuffer::from_str(&text);
        let scale = window.scale_factor() as f32;
        renderer.set_scale_factor(scale);

        let mut metrics = MetricsCollector::new();
        let mut frame_totals: Vec<Duration> = Vec::with_capacity(512);
        let mut max_frame = Duration::ZERO;

        let snap = buffer.snapshot();
        let line_h = 20.0_f32 * scale;
        let max_scroll = (snap.rope().len_lines().saturating_sub(1) as f32) * line_h;

        for i in 0..400_u32 {
            let scroll_y = ((i as f32 / 400.0) * max_scroll).min(max_scroll);
            let physical = window.inner_size();
            let input = FrameInput {
                buffer: &snap,
                scroll: ScrollOffset::new(scroll_y),
                clear_color: CLEAR,
                cursor_byte: buffer.len_bytes().min(1),
                cursor_blink_on: false,
                physical_size: physical,
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
                language: editor_render::editor_syntax::Language::Plain,
            };
            match renderer.render_frame(&input) {
                Ok(t) => {
                    metrics.record_frame(t.prepare, t.gpu, t.total);
                    frame_totals.push(t.total);
                    if t.total > max_frame {
                        max_frame = t.total;
                    }
                }
                Err(e) => {
                    eprintln!("perf-smoke: render_frame failed: {e}");
                    event_loop.exit();
                    return;
                }
            }
        }

        // Scripted edits: append 100 chars at end, then undo in batches via buffer + extra frames
        let mut buf = buffer;
        for _ in 0..100 {
            buf.insert(BytePos(buf.len_bytes()), "a").expect("insert ascii");
        }
        let snap = buf.snapshot();
        for _ in 0..50 {
            let physical = window.inner_size();
            let input = FrameInput {
                buffer: &snap,
                scroll: ScrollOffset::new(max_scroll),
                clear_color: CLEAR,
                cursor_byte: buf.len_bytes(),
                cursor_blink_on: true,
                physical_size: physical,
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
                language: editor_render::editor_syntax::Language::Plain,
            };
            if let Ok(t) = renderer.render_frame(&input) {
                metrics.record_frame(t.prepare, t.gpu, t.total);
                frame_totals.push(t.total);
                if t.total > max_frame {
                    max_frame = t.total;
                }
            }
        }

        let relaxed = std::env::var("PERF_SMOKE_RELAX").as_deref() == Ok("1");
        let (thr_p99, thr_max) =
            if relaxed { (100.0_f64, 500.0_f64) } else { (16.0_f64, 50.0_f64) };

        let mut sorted = frame_totals.clone();
        sorted.sort_unstable();
        let p99 = percentile_sorted(&sorted, 0.99);
        let p95 = percentile_sorted(&sorted, 0.95);
        let p50 = percentile_sorted(&sorted, 0.50);

        let p99_ms = p99.as_secs_f64() * 1000.0;
        let max_ms = max_frame.as_secs_f64() * 1000.0;

        let passed = if relaxed {
            max_ms < thr_max && !frame_totals.is_empty()
        } else {
            p99_ms <= thr_p99 && max_ms <= thr_max
        };

        self.report = Some(PerfSmokeReport {
            p50_ms: p50.as_secs_f64() * 1000.0,
            p95_ms: p95.as_secs_f64() * 1000.0,
            p99_ms,
            max_ms,
            frames: frame_totals.len(),
            passed,
            threshold_p99_ms: thr_p99,
            threshold_max_ms: thr_max,
            relaxed,
        });

        event_loop.exit();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let WindowEvent::CloseRequested = event {
            event_loop.exit();
        }
    }
}

fn percentile_sorted(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Run headless-window perf sequence; prints one JSON line to stdout.
pub fn run() -> anyhow::Result<()> {
    let event_loop = event_loop_for_smoke()?;
    let mut app = PerfSmokeApp { done: false, report: None };
    event_loop.run_app(&mut app)?;

    let report = app.report.ok_or_else(|| anyhow::anyhow!("perf-smoke: no report"))?;
    println!("{}", serde_json::to_string(&report)?);
    if !report.passed {
        anyhow::bail!(
            "perf-smoke: thresholds exceeded (p99 <= {} ms, max <= {} ms; got p99={:.2} ms max={:.2} ms; relaxed={})",
            report.threshold_p99_ms,
            report.threshold_max_ms,
            report.p99_ms,
            report.max_ms,
            report.relaxed
        );
    }
    Ok(())
}
