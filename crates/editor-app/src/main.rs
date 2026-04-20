//! `editor-app` — binary shell for the IDE project.
//!
//! M04+: `EditorRenderer` draws a [`TextBuffer`](editor_core::TextBuffer) via glyphon.
//!
//! See `docs/ARCHITECTURE.md` for wiring and `docs/MISSIONS.md` for the plan.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
// CLI help and parse errors are intentionally written to stderr (user-facing).
#![allow(clippy::print_stderr)]

mod config;
mod metrics;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use crossbeam_channel::Receiver;
use editor_core::{
    BytePos, Cursor, CursorMotion, EditKind, ScrollOffset, TextBuffer, UndoStack, WorkerPool,
};
use editor_input::{map_key_event, EditorCommand};
use editor_io::{load_file_sync, save_file_sync, Encoding, LoadError, LoadedFile, SaveError};
use tracing::{debug, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, PhysicalSize};
use winit::event::ElementState;
use winit::event::Ime;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::ModifiersState;
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

/// Files at or above this size are read on a background thread so the window can show immediately
/// (M06: avoid blocking the UI on huge reads).
const ASYNC_LOAD_MIN_BYTES: u64 = 4 * 1024 * 1024;

enum InitialLoadPlan {
    /// Bundled sample text (no path).
    Sample,
    /// Small/medium file loaded synchronously.
    Immediate(LoadedFile),
    /// Large file: load on worker after window shows.
    Deferred { path: String },
}

fn plan_initial_load(open_arg: Option<String>) -> InitialLoadPlan {
    match open_arg {
        None => InitialLoadPlan::Sample,
        Some(p) => {
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            if size >= ASYNC_LOAD_MIN_BYTES {
                InitialLoadPlan::Deferred { path: p }
            } else {
                match load_file_sync(Path::new(&p)) {
                    Ok(l) => InitialLoadPlan::Immediate(l),
                    Err(e) => {
                        eprintln!("editor-app: could not load {p}: {e}");
                        InitialLoadPlan::Sample
                    }
                }
            }
        }
    }
}

fn resolve_initial_plan(
    cli_path: Option<String>,
    persisted: &config::PersistedState,
) -> InitialLoadPlan {
    if let Some(p) = cli_path {
        if !p.starts_with('-') {
            return plan_initial_load(Some(p));
        }
    }
    if let Some(ref lf) = persisted.last_file {
        if lf.exists() {
            return plan_initial_load(Some(lf.to_string_lossy().into_owned()));
        }
        info!(?lf, "persisted last file missing on disk; opening bundled sample");
    }
    plan_initial_load(None)
}

fn restore_cursor_byte(
    open_path: &Option<PathBuf>,
    persisted: &config::PersistedState,
    len_bytes: usize,
) -> usize {
    let same = match (open_path, &persisted.last_file) {
        (Some(a), Some(b)) => a == b,
        (None, None) => true,
        _ => false,
    };
    if !same {
        return 0;
    }
    persisted.last_cursor_byte.map(|n| (n as usize).min(len_bytes)).unwrap_or(0)
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

    let persisted = config::PersistedState::load();
    let open_arg = args.iter().find(|a| !a.starts_with('-')).cloned();
    let plan = resolve_initial_plan(open_arg, &persisted);
    if let Err(e) = run_windowed(plan, persisted) {
        eprintln!("{e:#}");
        return Err(ExitCode::FAILURE);
    }
    Ok(())
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

fn run_windowed(plan: InitialLoadPlan, persisted: config::PersistedState) -> anyhow::Result<()> {
    info!(version = VERSION, "ide: starting windowed mode");
    info!("linked: {}", editor_core::banner());
    info!("linked: {}", editor_render::banner());
    info!("linked: {}", editor_input::banner());
    info!("linked: {}", editor_io::banner());
    info!("linked: {}", editor_ui::banner());

    let mut el_builder = EventLoop::<AppEvent>::with_user_event();
    let event_loop = el_builder.build()?;
    let proxy = event_loop.create_proxy();
    let proxy_blink = proxy.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(530));
        let _ = proxy_blink.send_event(AppEvent::BlinkTick);
    });

    let sample = include_str!("../assets/sample.txt");
    let worker_pool = WorkerPool::new(None);

    let (buffer, open_path, disk_encoding, file_mtime, document_loading, load_rx, cursor_byte0) =
        match plan {
            InitialLoadPlan::Sample => {
                let b = TextBuffer::from_str(sample);
                let cb = restore_cursor_byte(&None, &persisted, b.len_bytes());
                (b, None, Encoding::Utf8, None, false, None, cb)
            }
            InitialLoadPlan::Immediate(l) => {
                let path = Some(l.path.clone());
                let cb = restore_cursor_byte(&path, &persisted, l.buffer.len_bytes());
                let enc = l.encoding;
                let mt = Some(l.mtime);
                let buf = l.buffer;
                (buf, path, enc, mt, false, None, cb)
            }
            InitialLoadPlan::Deferred { path } => {
                let pb = PathBuf::from(&path);
                let open_path = Some(pb.clone());
                let b = TextBuffer::from_str(sample);
                let cb = restore_cursor_byte(&open_path, &persisted, b.len_bytes());
                let (_, rx) = worker_pool.spawn(move |_t| load_file_sync(&pb));
                (b, open_path, Encoding::Utf8, None, true, Some(rx), cb)
            }
        };

    let mut app = App {
        window: None,
        renderer: None,
        buffer,
        scroll: ScrollOffset::new(persisted.last_scroll_y.unwrap_or(0.0)),
        scale_factor: 1.0,
        cursor: Cursor::new(BytePos(cursor_byte0)),
        blink_on: true,
        undo: UndoStack::default(),
        modifiers: ModifiersState::default(),
        dev_hud: false,
        open_path,
        document_loading,
        worker_pool,
        disk_encoding,
        file_mtime,
        dirty: false,
        external_modified: false,
        load_rx,
        save_rx: None,
        persisted,
        metrics: metrics::MetricsCollector::new(),
        last_metrics_debug: Instant::now() - Duration::from_secs(2),
        ime_suppress_next_keytext: false,
    };
    app.clamp_cursor_to_buffer();
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
    open_path: Option<PathBuf>,
    /// `true` while a file load is in flight on a worker thread.
    document_loading: bool,
    worker_pool: WorkerPool,
    disk_encoding: Encoding,
    file_mtime: Option<SystemTime>,
    dirty: bool,
    external_modified: bool,
    load_rx: Option<Receiver<Result<LoadedFile, LoadError>>>,
    save_rx: Option<Receiver<Result<(), SaveError>>>,
    persisted: config::PersistedState,
    metrics: metrics::MetricsCollector,
    last_metrics_debug: Instant,
    /// After [`Ime::Commit`], drop one duplicate [`EditorCommand::InsertText`] from the next key event.
    ime_suppress_next_keytext: bool,
}

impl App {
    /// Bottom status bar height in physical pixels (matches `editor-ui` / `TextLayer`).
    fn status_bar_height_px(&self) -> f32 {
        24.0 * self.scale_factor
    }

    /// Viewport height available for the text canvas (window minus status bar).
    fn content_height_for_layout(&self) -> f32 {
        let Some(w) = self.window.as_ref() else {
            return 1.0;
        };
        (w.inner_size().height as f32 - self.status_bar_height_px()).max(1.0)
    }

    fn visible_line_count(&self) -> usize {
        let Some(renderer) = self.renderer.as_ref() else {
            return 1;
        };
        if self.window.is_none() {
            return 1;
        }
        let line_h = renderer.line_height_px();
        let h = self.content_height_for_layout();
        (h / line_h).floor().max(1.0) as usize
    }

    fn clamp_scroll(&mut self) {
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let h = self.content_height_for_layout();
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
        let line_h = renderer.line_height_px();
        let h = self.content_height_for_layout();
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

    fn clamp_cursor_to_buffer(&mut self) {
        let mut p = self.cursor.pos().0.min(self.buffer.len_bytes());
        while p > 0 && !self.buffer.is_char_boundary(BytePos(p)) {
            p -= 1;
        }
        self.cursor = Cursor::new(BytePos(p));
    }

    fn poll_io(&mut self) {
        if let Some(rx) = &self.load_rx {
            if let Ok(res) = rx.try_recv() {
                self.load_rx = None;
                self.document_loading = false;
                match res {
                    Ok(l) => self.apply_loaded(l),
                    Err(e) => tracing::warn!(error = %e, "file load failed"),
                }
                self.sync_window_title();
                self.request_redraw();
            }
        }
        if let Some(rx) = &self.save_rx {
            if let Ok(res) = rx.try_recv() {
                self.save_rx = None;
                match res {
                    Ok(()) => {
                        self.dirty = false;
                        if let Some(ref p) = self.open_path {
                            if let Ok(m) = std::fs::metadata(p).and_then(|x| x.modified()) {
                                self.file_mtime = Some(m);
                            }
                        }
                        self.external_modified = false;
                    }
                    Err(e) => tracing::warn!(error = %e, "save failed"),
                }
                self.sync_window_title();
            }
        }
    }

    fn apply_loaded(&mut self, l: LoadedFile) {
        self.buffer = l.buffer;
        self.open_path = Some(l.path);
        self.disk_encoding = l.encoding;
        self.file_mtime = Some(l.mtime);
        self.cursor = Cursor::new(BytePos::ZERO);
        self.scroll = ScrollOffset::default();
        self.undo = UndoStack::default();
        self.dirty = false;
        self.external_modified = false;
        self.clamp_cursor_to_buffer();
    }

    fn start_background_load(&mut self, path: PathBuf) {
        if self.load_rx.is_some() {
            return;
        }
        let (_, rx) = self.worker_pool.spawn(move |_t| load_file_sync(&path));
        self.load_rx = Some(rx);
        self.document_loading = true;
    }

    fn open_via_dialog(&mut self) {
        if self.dirty {
            tracing::warn!("opening another file with unsaved changes (no save prompt yet)");
        }
        if let Some(p) = rfd::FileDialog::new().pick_file() {
            self.start_background_load(p);
            self.sync_window_title();
        }
    }

    fn save_via_dialog_or_disk(&mut self) {
        if self.save_rx.is_some() {
            return;
        }
        let path = if let Some(ref p) = self.open_path {
            p.clone()
        } else if let Some(p) = rfd::FileDialog::new().save_file() {
            self.open_path = Some(p.clone());
            p
        } else {
            return;
        };
        let snap = self.buffer.snapshot();
        let le = self.buffer.original_line_ending();
        let enc = self.disk_encoding;
        let (_, rx) = self.worker_pool.spawn(move |_t| save_file_sync(&path, &snap, le, enc));
        self.save_rx = Some(rx);
    }

    /// Writes `state.json` (cursor, scroll, window geometry, last file) for next launch (M10).
    fn persist_session(&mut self) {
        self.persisted.last_file = self.open_path.clone();
        self.persisted.last_cursor_byte = Some(self.cursor.pos().0 as u64);
        self.persisted.last_scroll_y = Some(self.scroll.y_px);
        if let Some(w) = &self.window {
            let s = w.inner_size();
            self.persisted.window_size = Some((s.width, s.height));
            if let Ok(pos) = w.outer_position() {
                self.persisted.window_pos = Some((pos.x, pos.y));
            }
        }
        if let Err(e) = self.persisted.save() {
            tracing::warn!(error = %e, "could not persist session");
        }
    }

    fn frame_status(&self) -> editor_ui::StatusBarInfo {
        let (cursor_line, cursor_col) = self
            .buffer
            .byte_to_line_col(BytePos(self.cursor.pos().0.min(self.buffer.len_bytes())))
            .map(|lc| (lc.line, lc.col))
            .unwrap_or((0, 0));
        let enc = match self.disk_encoding {
            Encoding::Utf8 | Encoding::LossyUtf8 => editor_ui::SourceEncoding::Utf8,
            Encoding::Utf8Bom => editor_ui::SourceEncoding::Utf8Bom,
            Encoding::Utf16Le => editor_ui::SourceEncoding::Utf16Le,
            Encoding::Utf16Be => editor_ui::SourceEncoding::Utf16Be,
        };
        editor_ui::StatusBarInfo {
            path: self.open_path.clone(),
            dirty: self.dirty,
            cursor_line,
            cursor_col,
            total_lines: self.buffer.len_lines(),
            encoding: enc,
            line_ending: self.buffer.original_line_ending(),
            external_modified: self.external_modified,
        }
    }

    /// Full frame: shape visible text, submit GPU work, present.
    ///
    /// Called from [`WindowEvent::RedrawRequested`]. Also invoked directly on
    /// [`WindowEvent::Resized`] / [`WindowEvent::ScaleFactorChanged`] so the
    /// window keeps painting during OS modal resize (notably Windows), where
    /// redraw requests may not drain until the user releases the drag edge.
    fn paint_frame(&mut self) {
        self.poll_io();
        let status = self.frame_status();
        let dev_hud_line = self.dev_hud.then(|| self.metrics.hud_line());
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
                status: Some(status),
                dev_hud_line,
            };
            match renderer.render_frame(&input) {
                Ok(timings) => {
                    self.metrics.record_frame(timings.prepare, timings.gpu, timings.total);
                    if self.last_metrics_debug.elapsed() >= Duration::from_secs(2) {
                        debug!(snapshot = ?self.metrics.snapshot(), "metrics");
                        self.last_metrics_debug = Instant::now();
                    }
                    #[cfg(debug_assertions)]
                    {
                        if timings.prepare > Duration::from_millis(4) {
                            tracing::warn!(?timings.prepare, "text prepare exceeded 4ms budget");
                        }
                        if timings.gpu > Duration::from_millis(6) {
                            tracing::warn!(?timings.gpu, "GPU submit exceeded 6ms budget");
                        }
                        if timings.total > Duration::from_millis(16) {
                            tracing::warn!(?timings.total, "frame paint exceeded 16ms budget");
                        }
                    }
                }
                Err(e) => tracing::warn!(error = %e, "render frame"),
            }
        }
    }

    fn sync_window_title(&self) {
        let Some(window) = &self.window else {
            return;
        };
        if self.document_loading {
            if let Some(ref p) = self.open_path {
                window.set_title(&format!("IDE — Loading… — {}", p.display()));
            } else {
                window.set_title("IDE — Loading…");
            }
            return;
        }
        let dirty = if self.dirty { "*" } else { "" };
        if self.dev_hud {
            window.set_title(&format!(
                "IDE{dirty} | lines={} bytes={} caret={} undo={}",
                self.buffer.len_lines(),
                self.buffer.len_bytes(),
                self.cursor.pos().0,
                self.undo.len_undo(),
            ));
        } else if let Some(ref p) = self.open_path {
            window.set_title(&format!("IDE{dirty} — {}", p.display()));
        } else {
            window.set_title(&format!("IDE{dirty} — sample.txt (bundled)"));
        }
    }

    fn apply_editor_command(&mut self, cmd: EditorCommand) -> bool {
        match cmd {
            EditorCommand::Quit => {
                self.persist_session();
                true
            }
            EditorCommand::Open => {
                self.open_via_dialog();
                false
            }
            EditorCommand::InsertText(text) => {
                if !text.is_empty() {
                    let pos = self.cursor.pos().0;
                    if let Ok(edit) = self
                        .buffer
                        .apply_edit(EditKind::Insert { pos: BytePos(pos), text: text.clone() })
                    {
                        self.undo.push(edit);
                        self.cursor = Cursor::new(BytePos(pos + text.len()));
                        self.dirty = true;
                        self.scroll_cursor_into_view();
                    }
                }
                false
            }
            EditorCommand::InsertNewline => {
                let pos = self.cursor.pos().0;
                if let Ok(edit) = self
                    .buffer
                    .apply_edit(EditKind::Insert { pos: BytePos(pos), text: "\n".into() })
                {
                    self.undo.push(edit);
                    self.cursor = Cursor::new(BytePos(pos + 1));
                    self.dirty = true;
                    self.scroll_cursor_into_view();
                }
                false
            }
            EditorCommand::DeleteBackward => {
                let end = self.cursor.pos().0;
                if end > 0 {
                    let mut c = self.cursor;
                    if c.apply(CursorMotion::Left, &self.buffer).is_ok() {
                        let start = c.pos().0;
                        if let Ok(deleted) =
                            self.buffer.slice_to_string(BytePos(start)..BytePos(end))
                        {
                            if let Ok(edit) = self.buffer.apply_edit(EditKind::Delete {
                                range: BytePos(start)..BytePos(end),
                                deleted_text: deleted,
                            }) {
                                self.undo.push(edit);
                                self.cursor = c;
                                self.dirty = true;
                                self.scroll_cursor_into_view();
                            }
                        }
                    }
                }
                false
            }
            EditorCommand::DeleteForward => {
                let start = self.cursor.pos().0;
                if start < self.buffer.len_bytes() {
                    let mut c = self.cursor;
                    if c.apply(CursorMotion::Right, &self.buffer).is_ok() {
                        let end = c.pos().0;
                        if let Ok(deleted) =
                            self.buffer.slice_to_string(BytePos(start)..BytePos(end))
                        {
                            if let Ok(edit) = self.buffer.apply_edit(EditKind::Delete {
                                range: BytePos(start)..BytePos(end),
                                deleted_text: deleted,
                            }) {
                                self.undo.push(edit);
                                self.dirty = true;
                                self.scroll_cursor_into_view();
                            }
                        }
                    }
                }
                false
            }
            EditorCommand::Undo => {
                if let Ok(Some(_)) = self.undo.undo(&mut self.buffer) {
                    self.clamp_cursor_to_buffer();
                    self.dirty = true;
                    self.scroll_cursor_into_view();
                }
                false
            }
            EditorCommand::Redo => {
                if let Ok(Some(_)) = self.undo.redo(&mut self.buffer) {
                    self.clamp_cursor_to_buffer();
                    self.dirty = true;
                    self.scroll_cursor_into_view();
                }
                false
            }
            EditorCommand::Save => {
                self.save_via_dialog_or_disk();
                false
            }
            EditorCommand::PageUp => {
                self.page_vertical(-1);
                false
            }
            EditorCommand::PageDown => {
                self.page_vertical(1);
                false
            }
            EditorCommand::ToggleDevHud => {
                self.dev_hud = !self.dev_hud;
                info!(dev_hud = self.dev_hud, "dev HUD (title bar) toggled");
                false
            }
            EditorCommand::ApplyCursorMotion { motion, extend_selection: _ } => {
                if self.cursor.apply(motion, &self.buffer).is_ok() {
                    self.scroll_cursor_into_view();
                }
                false
            }
            EditorCommand::DeleteWordBackward => {
                let s = self.buffer.to_text();
                let pos = self.cursor.pos().0;
                if let Some(r) = editor_core::delete_word_backward_range(&s, pos) {
                    let deleted = s[r.start..r.end].to_string();
                    if let Ok(edit) = self.buffer.apply_edit(EditKind::Delete {
                        range: BytePos(r.start)..BytePos(r.end),
                        deleted_text: deleted,
                    }) {
                        self.undo.push(edit);
                        self.cursor = Cursor::new(BytePos(r.start));
                        self.dirty = true;
                        self.scroll_cursor_into_view();
                    }
                }
                false
            }
            EditorCommand::DeleteWordForward => {
                let s = self.buffer.to_text();
                let pos = self.cursor.pos().0;
                if let Some(r) = editor_core::delete_word_forward_range(&s, pos) {
                    let deleted = s[r.start..r.end].to_string();
                    if let Ok(edit) = self.buffer.apply_edit(EditKind::Delete {
                        range: BytePos(r.start)..BytePos(r.end),
                        deleted_text: deleted,
                    }) {
                        self.undo.push(edit);
                        self.cursor = Cursor::new(BytePos(r.start));
                        self.dirty = true;
                        self.scroll_cursor_into_view();
                    }
                }
                false
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
        let mut attrs = Window::default_attributes().with_title("IDE");
        if let Some((w, h)) = self.persisted.window_size {
            attrs = attrs.with_inner_size(PhysicalSize::new(w, h));
        }
        if let Some((x, y)) = self.persisted.window_pos {
            attrs = attrs.with_position(LogicalPosition::new(x, y));
        }
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let scale = window.scale_factor() as f32;
        self.scale_factor = scale;
        let mut renderer =
            editor_render::EditorRenderer::new(window.clone()).expect("init GPU + text");
        renderer.set_scale_factor(scale);
        window.set_ime_allowed(true);
        window.request_redraw();
        self.window = Some(window);
        self.renderer = Some(renderer);
        self.sync_window_title();
        self.clamp_scroll();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.persist_session();
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
                if let (Some(w), Some(r)) = (&self.window, self.renderer.as_mut()) {
                    r.sync_present_mode(w);
                    r.set_scale_factor(self.scale_factor);
                }
                self.paint_frame();
            }
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(r)) = (&self.window, self.renderer.as_mut()) {
                    r.resize(size);
                    r.sync_present_mode(w);
                }
                self.clamp_scroll();
                self.paint_frame();
            }
            WindowEvent::Moved(_) => {
                if let (Some(w), Some(r)) = (&self.window, self.renderer.as_mut()) {
                    r.sync_present_mode(w);
                }
                self.request_redraw();
            }
            WindowEvent::Focused(true) => {
                if let (Some(ref p), Some(remembered)) = (&self.open_path, self.file_mtime) {
                    if let Ok(m) = std::fs::metadata(p).and_then(|x| x.modified()) {
                        if m != remembered {
                            self.external_modified = true;
                            tracing::warn!(path = %p.display(), "file changed on disk");
                        }
                    }
                }
                self.request_redraw();
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
            }
            WindowEvent::Ime(ime) => match ime {
                Ime::Enabled | Ime::Disabled => {}
                Ime::Preedit(_, _) => {
                    self.request_redraw();
                }
                Ime::Commit(text) => {
                    if !text.is_empty() && !self.document_loading {
                        self.ime_suppress_next_keytext = true;
                        let _ = self.apply_editor_command(EditorCommand::InsertText(text));
                        self.sync_window_title();
                        self.request_redraw();
                    }
                }
            },
            WindowEvent::KeyboardInput { event, is_synthetic, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                if is_synthetic {
                    return;
                }
                if let Some(cmd) = map_key_event(&event, self.modifiers) {
                    if self.ime_suppress_next_keytext {
                        if matches!(&cmd, EditorCommand::InsertText(_)) {
                            self.ime_suppress_next_keytext = false;
                            return;
                        }
                        self.ime_suppress_next_keytext = false;
                    }
                    let quit = self.apply_editor_command(cmd);
                    self.sync_window_title();
                    self.request_redraw();
                    if quit {
                        event_loop.exit();
                    }
                }
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
