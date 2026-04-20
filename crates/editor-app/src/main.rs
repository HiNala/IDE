//! `editor-app` — binary shell for the IDE project.
//!
//! M04+: `EditorRenderer` draws a [`TextBuffer`](editor_core::TextBuffer) via glyphon.
//!
//! See `docs/ARCHITECTURE.md` for wiring and `docs/MISSIONS.md` for the plan.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
// CLI help and parse errors are intentionally written to stderr (user-facing).
#![allow(clippy::print_stderr)]

use std::process::ExitCode;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use editor_core::{BytePos, Cursor, CursorMotion, EditKind, ScrollOffset, TextBuffer, UndoStack};
use editor_input::{map_key_event, EditorCommand};
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use winit::application::ApplicationHandler;
use winit::event::ElementState;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

/// Crate / app version from `Cargo.toml`.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Dark background (`#1e1e1e`).
const CLEAR: wgpu::Color = wgpu::Color { r: 0.118, g: 0.118, b: 0.118, a: 1.0 };

#[derive(Debug)]
enum AppEvent {
    /// Toggle cursor blink phase (~2 Hz).
    BlinkTick,
}

fn main() -> ExitCode {
    init_tracing();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

/// Reserved exit codes: `0` OK, `2` unrecoverable GPU (future), `64` bad CLI (BSD convention).
fn run() -> Result<(), ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    for a in &args {
        if a == "--help" || a == "-h" {
            print_help();
            return Ok(());
        }
        if a.starts_with("--") && a != "--dry-run" {
            eprintln!("editor-app: unknown option: {a}");
            return Err(ExitCode::from(64));
        }
    }

    if args.iter().any(|a| a == "--dry-run") {
        if let Err(e) = run_dry_run() {
            eprintln!("{e:#}");
            return Err(ExitCode::FAILURE);
        }
        return Ok(());
    }

    let (initial, open_path) = initial_document(&args);
    if let Err(e) = run_windowed(&initial, open_path) {
        eprintln!("{e:#}");
        return Err(ExitCode::FAILURE);
    }
    Ok(())
}

fn initial_document(args: &[String]) -> (String, Option<String>) {
    let path = args.iter().find(|a| !a.starts_with('-'));
    match path {
        Some(p) => match std::fs::read_to_string(p) {
            Ok(s) => (s, Some(p.clone())),
            Err(e) => {
                eprintln!("editor-app: could not read {p}: {e}");
                (include_str!("../assets/sample.txt").to_string(), None)
            }
        },
        None => (include_str!("../assets/sample.txt").to_string(), None),
    }
}

fn print_help() {
    eprintln!(
        "\
editor-app — IDE binary (MVP in progress)

Usage:
  editor-app [path/to/file.txt] [--dry-run] [--help]

Arguments:
  path        Optional UTF-8 text file to open (falls back to bundled sample on error).

Options:
  --dry-run   Headless GPU adapter/device init (no window).
  -h, --help  Show this help.
"
    );
}

fn run_dry_run() -> anyhow::Result<()> {
    info!(version = VERSION, "ide: dry-run (headless GPU init)");
    editor_render::dry_run_headless()?;
    info!("ide: dry-run OK");
    Ok(())
}

fn run_windowed(initial: &str, open_path: Option<String>) -> anyhow::Result<()> {
    info!(version = VERSION, "ide: starting windowed mode");
    info!("linked: {}", editor_core::banner());
    info!("linked: {}", editor_render::banner());
    info!("linked: {}", editor_input::banner());
    info!("linked: {}", editor_io::banner());
    info!("linked: {}", editor_ui::banner());

    let mut el_builder = EventLoop::<AppEvent>::with_user_event();
    let event_loop = el_builder.build()?;
    let proxy = event_loop.create_proxy();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(530));
        let _ = proxy.send_event(AppEvent::BlinkTick);
    });

    let mut app = App {
        window: None,
        renderer: None,
        buffer: TextBuffer::from_str(initial),
        scroll: ScrollOffset::default(),
        scale_factor: 1.0,
        cursor: Cursor::new(BytePos::ZERO),
        blink_on: true,
        undo: UndoStack::default(),
        modifiers: ModifiersState::default(),
        dev_hud: false,
        open_path,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<editor_render::EditorRenderer>,
    buffer: TextBuffer,
    scroll: ScrollOffset,
    scale_factor: f32,
    cursor: Cursor,
    /// Caret opacity phase (toggled by [`AppEvent::BlinkTick`]).
    blink_on: bool,
    undo: UndoStack,
    modifiers: ModifiersState,
    /// F11: show buffer stats in the window title.
    dev_hud: bool,
    /// User-supplied file path from CLI, if any (for title bar).
    open_path: Option<String>,
}

impl App {
    fn visible_line_count(&self) -> usize {
        let Some(renderer) = self.renderer.as_ref() else {
            return 1;
        };
        let Some(w) = self.window.as_ref() else {
            return 1;
        };
        let line_h = renderer.line_height_px();
        let h = w.inner_size().height.max(1) as f32;
        (h / line_h).floor().max(1.0) as usize
    }

    fn clamp_scroll(&mut self) {
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let Some(window) = &self.window else {
            return;
        };
        let h = window.inner_size().height.max(1) as f32;
        let line_h = renderer.line_height_px();
        let total_lines = self.buffer.len_lines();
        let content_h = total_lines as f32 * line_h;
        let max_scroll = (content_h - h).max(0.0);
        self.scroll.y_px = self.scroll.y_px.clamp(0.0, max_scroll);
    }

    fn scroll_cursor_into_view(&mut self) {
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let line_h = renderer.line_height_px();
        let h = window.inner_size().height.max(1) as f32;
        let byte = self.cursor.pos().0.min(self.buffer.len_bytes());
        let Ok(lc) = self.buffer.byte_to_line_col(BytePos(byte)) else {
            return;
        };
        let cursor_line = lc.line;
        let first = (self.scroll.y_px / line_h).floor() as usize;
        let visible = (h / line_h).floor().max(1.0) as usize;
        let last = first + visible.saturating_sub(1);

        if cursor_line < first {
            self.scroll.y_px = cursor_line as f32 * line_h;
        } else if cursor_line > last {
            self.scroll.y_px = (cursor_line + 1) as f32 * line_h - h;
        }
        self.clamp_scroll();
    }

    fn line_vertical(&mut self, dir: isize) {
        let motion = if dir > 0 { CursorMotion::Down } else { CursorMotion::Up };
        let _ = self.cursor.apply(motion, &self.buffer);
        self.scroll_cursor_into_view();
    }

    fn line_horizontal(&mut self, dir: isize) {
        let motion = if dir > 0 { CursorMotion::Right } else { CursorMotion::Left };
        let _ = self.cursor.apply(motion, &self.buffer);
        self.scroll_cursor_into_view();
    }

    fn page_vertical(&mut self, dir: isize) {
        let n = self.visible_line_count().max(1);
        let motion = if dir > 0 { CursorMotion::Down } else { CursorMotion::Up };
        for _ in 0..n {
            let _ = self.cursor.apply(motion, &self.buffer);
        }
        self.scroll_cursor_into_view();
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Full frame: shape visible text, submit GPU work, present.
    ///
    /// Called from [`WindowEvent::RedrawRequested`]. Also invoked directly on
    /// [`WindowEvent::Resized`] / [`WindowEvent::ScaleFactorChanged`] so the
    /// window keeps painting during OS modal resize (notably Windows), where
    /// redraw requests may not drain until the user releases the drag edge.
    fn paint_frame(&mut self) {
        if let (Some(renderer), Some(w)) = (self.renderer.as_mut(), self.window.as_ref()) {
            let snap = self.buffer.snapshot();
            let physical = w.inner_size();
            let input = editor_render::FrameInput {
                buffer: &snap,
                scroll: self.scroll,
                clear_color: CLEAR,
                cursor_byte: self.cursor.pos().0.min(self.buffer.len_bytes()),
                cursor_blink_on: self.blink_on,
                physical_size: physical,
                scale_factor: self.scale_factor,
            };
            if let Err(e) = renderer.render_frame(&input) {
                tracing::warn!(error = %e, "render frame");
            }
        }
    }

    fn sync_window_title(&self) {
        let Some(window) = &self.window else {
            return;
        };
        if self.dev_hud {
            window.set_title(&format!(
                "IDE | lines={} bytes={} caret={} undo={}",
                self.buffer.len_lines(),
                self.buffer.len_bytes(),
                self.cursor.pos().0,
                self.undo.len_undo(),
            ));
        } else if let Some(ref p) = self.open_path {
            window.set_title(&format!("IDE — {p}"));
        } else {
            window.set_title("IDE — sample.txt (bundled)");
        }
    }

    fn apply_editor_command(&mut self, cmd: EditorCommand) {
        match cmd {
            EditorCommand::ToggleDevHud => {
                self.dev_hud = !self.dev_hud;
                info!(dev_hud = self.dev_hud, "dev HUD (title bar) toggled");
            }
            EditorCommand::ApplyCursorMotion { motion, extend_selection: _ } => {
                if self.cursor.apply(motion, &self.buffer).is_err() {
                    return;
                }
                self.scroll_cursor_into_view();
            }
            EditorCommand::DeleteWordBackward => {
                let s = self.buffer.to_text();
                let pos = self.cursor.pos().0;
                let Some(r) = editor_core::delete_word_backward_range(&s, pos) else {
                    return;
                };
                let deleted = s[r.start..r.end].to_string();
                let Ok(edit) = self.buffer.apply_edit(EditKind::Delete {
                    range: BytePos(r.start)..BytePos(r.end),
                    deleted_text: deleted,
                }) else {
                    return;
                };
                self.undo.push(edit);
                self.cursor = Cursor::new(BytePos(r.start));
                self.scroll_cursor_into_view();
            }
            EditorCommand::DeleteWordForward => {
                let s = self.buffer.to_text();
                let pos = self.cursor.pos().0;
                let Some(r) = editor_core::delete_word_forward_range(&s, pos) else {
                    return;
                };
                let deleted = s[r.start..r.end].to_string();
                let Ok(edit) = self.buffer.apply_edit(EditKind::Delete {
                    range: BytePos(r.start)..BytePos(r.end),
                    deleted_text: deleted,
                }) else {
                    return;
                };
                self.undo.push(edit);
                self.cursor = Cursor::new(BytePos(r.start));
                self.scroll_cursor_into_view();
            }
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::BlinkTick => {
                self.blink_on = !self.blink_on;
                self.request_redraw();
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("IDE"))
                .expect("create window"),
        );
        let scale = window.scale_factor() as f32;
        self.scale_factor = scale;
        let mut renderer =
            editor_render::EditorRenderer::new(window.clone()).expect("init GPU + text");
        renderer.set_scale_factor(scale);
        window.request_redraw();
        self.window = Some(window);
        self.renderer = Some(renderer);
        self.sync_window_title();
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
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale_factor = scale_factor as f32;
                if let Some(r) = self.renderer.as_mut() {
                    r.set_scale_factor(self.scale_factor);
                }
                self.paint_frame();
            }
            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size);
                }
                self.clamp_scroll();
                self.paint_frame();
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
            }
            WindowEvent::KeyboardInput { event, is_synthetic, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                if is_synthetic {
                    return;
                }
                if let Some(cmd) = map_key_event(&event, self.modifiers) {
                    self.apply_editor_command(cmd);
                    self.sync_window_title();
                    self.request_redraw();
                    return;
                }

                let nav = matches!(
                    event.physical_key,
                    PhysicalKey::Code(KeyCode::ArrowDown)
                        | PhysicalKey::Code(KeyCode::ArrowUp)
                        | PhysicalKey::Code(KeyCode::ArrowLeft)
                        | PhysicalKey::Code(KeyCode::ArrowRight)
                        | PhysicalKey::Code(KeyCode::PageDown)
                        | PhysicalKey::Code(KeyCode::PageUp)
                );
                if !nav && event.repeat {
                    return;
                }

                match event.physical_key {
                    PhysicalKey::Code(KeyCode::ArrowDown) => self.line_vertical(1),
                    PhysicalKey::Code(KeyCode::ArrowUp) => self.line_vertical(-1),
                    PhysicalKey::Code(KeyCode::ArrowRight) => self.line_horizontal(1),
                    PhysicalKey::Code(KeyCode::ArrowLeft) => self.line_horizontal(-1),
                    PhysicalKey::Code(KeyCode::PageDown) => self.page_vertical(1),
                    PhysicalKey::Code(KeyCode::PageUp) => self.page_vertical(-1),
                    _ => return,
                }
                self.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.paint_frame();
            }
            _ => {}
        }
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,editor_app=info,editor_render=info,wgpu=warn"));

    #[cfg(feature = "tracy")]
    {
        if cfg!(debug_assertions) {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_tracy::TracyLayer::default())
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true).pretty())
                .init();
        } else {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_tracy::TracyLayer::default())
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true).compact())
                .init();
        }
    }

    #[cfg(not(feature = "tracy"))]
    {
        if cfg!(debug_assertions) {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true).pretty())
                .init();
        } else {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true).compact())
                .init();
        }
    }

    info!("editor-app v{VERSION} starting");
}
