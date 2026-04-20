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
    BytePos, Cursor, CursorMotion, EditKind, ScrollOffset, Selection, TextBuffer, UndoStack,
    WorkerPool,
};
use editor_input::{map_key_event, scroll_delta_y_pixels, EditorCommand, MouseChordState};
use editor_io::{load_file_sync, save_file_sync, Encoding, LoadError, LoadedFile, SaveError};
use tracing::{debug, info, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use winit::event::ElementState;
use winit::event::Ime;
use winit::event::MouseButton;
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
    let log_json = std::env::args().skip(1).any(|a| a == "--log-json");
    init_tracing(log_json);
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
        if a.starts_with("--") && !matches!(a.as_str(), "--dry-run" | "--log-json" | "--dev-hud") {
            eprintln!("editor-app: unknown option: {a}");
            return Err(ExitCode::from(64));
        }
    }

    let start_dev_hud = args.iter().any(|a| a == "--dev-hud");

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
    if let Err(e) = run_windowed(plan, persisted, start_dev_hud) {
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
  editor-app [path/to/file.txt] [--dry-run] [--dev-hud] [--help]

Arguments:
  path        Optional UTF-8 text file to open (falls back to bundled sample on error).

Options:
  --dry-run   Headless GPU adapter/device init (no window).
  --dev-hud   Start with the F11 metrics overlay visible (same as pressing F11).
  --log-json  Emit tracing logs as JSON lines (for tooling); still obeys RUST_LOG.
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

fn run_windowed(
    plan: InitialLoadPlan,
    persisted: config::PersistedState,
    start_dev_hud: bool,
) -> anyhow::Result<()> {
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
        dev_hud: start_dev_hud,
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
        selection: Selection::empty(BytePos(cursor_byte0)),
        mouse_chord: MouseChordState::default(),
        last_pointer: PhysicalPosition::new(0.0, 0.0),
        drag_anchor: None,
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
    /// Single anchor/head region; collapsed when anchor == head (caret only).
    selection: Selection,
    /// Multi-click + drag tracking (M09).
    mouse_chord: MouseChordState,
    /// Latest pointer position in physical pixels (for [`WindowEvent::MouseInput`] which has no coords).
    last_pointer: PhysicalPosition<f64>,
    /// Byte where a simple-click drag started (anchor for drag selection).
    drag_anchor: Option<BytePos>,
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
        self.selection = Selection::empty(self.cursor.pos());
    }

    fn collapse_selection_to_cursor(&mut self) {
        self.selection = Selection::empty(self.cursor.pos());
    }

    /// Deletes the selected range in one undo step; caret moves to range start.
    fn delete_selection_if_nonempty(&mut self) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        let r = self.selection.range();
        let Ok(deleted) = self.buffer.slice_to_string(r.start..r.end) else {
            return false;
        };
        let Ok(edit) = self
            .buffer
            .apply_edit(EditKind::Delete { range: r.start..r.end, deleted_text: deleted })
        else {
            return false;
        };
        self.undo.push(edit);
        self.cursor = Cursor::new(r.start);
        self.selection = Selection::empty(r.start);
        self.dirty = true;
        true
    }

    fn insert_string(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.delete_selection_if_nonempty();
        let pos = self.cursor.pos().0;
        if let Ok(edit) =
            self.buffer.apply_edit(EditKind::Insert { pos: BytePos(pos), text: text.to_string() })
        {
            self.undo.push(edit);
            self.cursor = Cursor::new(BytePos(pos + text.len()));
            self.selection = Selection::empty(self.cursor.pos());
            self.dirty = true;
            self.scroll_cursor_into_view();
        }
    }

    fn clipboard_copy_selection(&self) {
        if self.selection.is_empty() {
            return;
        }
        let Ok(t) = self.buffer.slice_to_string(self.selection.range()) else {
            return;
        };
        match arboard::Clipboard::new() {
            Ok(mut c) => {
                if let Err(e) = c.set_text(t) {
                    warn!(error = %e, "clipboard set");
                }
            }
            Err(e) => warn!(error = %e, "clipboard unavailable"),
        }
    }

    fn clipboard_cut(&mut self) {
        self.clipboard_copy_selection();
        let _ = self.delete_selection_if_nonempty();
    }

    fn clipboard_paste(&mut self) {
        let text = match arboard::Clipboard::new() {
            Ok(mut c) => match c.get_text() {
                Ok(t) => t,
                Err(e) => {
                    warn!(error = %e, "clipboard read");
                    return;
                }
            },
            Err(e) => {
                warn!(error = %e, "clipboard unavailable");
                return;
            }
        };
        self.insert_string(&text);
    }

    fn select_all(&mut self) {
        let len = self.buffer.len_bytes();
        self.selection = Selection { anchor: BytePos(0), head: BytePos(len) };
        self.cursor = Cursor::new(BytePos(len));
        self.scroll_cursor_into_view();
    }

    /// Bottom Y of the text region in physical pixels (content above the status bar).
    fn text_content_bottom_px(&self) -> Option<f64> {
        let w = self.window.as_ref()?;
        Some(f64::from(w.inner_size().height) - f64::from(self.status_bar_height_px()))
    }

    /// Scroll by one line when the pointer sits in the top/bottom margin during drag (M09 §6.8).
    fn autoscroll_drag_edges(&mut self, y_px: f64) {
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let Some(content_bottom) = self.text_content_bottom_px() else {
            return;
        };
        if content_bottom <= 1.0 {
            return;
        }
        let line_h = renderer.line_height_px();
        let edge = 20.0_f64.min(content_bottom / 3.0).max(4.0);
        if y_px < edge {
            self.scroll.y_px = (self.scroll.y_px - line_h).max(0.0);
        } else if y_px > content_bottom - edge {
            self.scroll.y_px += line_h;
        } else {
            return;
        }
        self.clamp_scroll();
    }

    /// Map physical window pixel to a UTF-8 boundary byte offset (M09; matches `editor-render` layout).
    fn hit_test_byte(&self, x_px: f64, y_px: f64) -> Option<BytePos> {
        let renderer = self.renderer.as_ref()?;
        let w = self.window.as_ref()?;
        let physical = w.inner_size();
        let line_h = renderer.line_height_px();
        let status_h = self.status_bar_height_px();
        let (gutter_w, char_w) =
            editor_render::compute_gutter_width_px(self.buffer.len_lines(), self.scale_factor);
        let content_bottom = physical.height as f32 - status_h;
        if y_px < 0.0 || y_px >= content_bottom as f64 {
            return None;
        }
        let total_lines = self.buffer.len_lines();
        if total_lines == 0 {
            return Some(BytePos(0));
        }
        let y = y_px as f32;
        let line_idx_f = (y - 4.0 + self.scroll.y_px) / line_h;
        let mut line_idx = line_idx_f.floor() as isize;
        if line_idx < 0 {
            line_idx = 0;
        }
        let mut line_idx = line_idx as usize;
        if line_idx >= total_lines {
            line_idx = total_lines.saturating_sub(1);
        }
        let body_left = 8.0 + gutter_w;
        let dx = x_px as f32 - body_left;
        let line_start = self.buffer.line_to_byte(line_idx).ok()?;
        let line_len = self.buffer.line_len_bytes(line_idx).ok()?;
        let col_byte = if dx <= 0.0 { 0usize } else { (dx / char_w.max(1e-6)).floor() as usize };
        let col_byte = col_byte.min(line_len);
        let mut byte = line_start + col_byte;
        while byte > line_start && !self.buffer.is_char_boundary(BytePos(byte)) {
            byte -= 1;
        }
        Some(BytePos(byte))
    }

    fn apply_mouse_click(&mut self, x_px: i32, y_px: i32, click_count: u8, shift: bool) {
        if self.document_loading {
            return;
        }
        let x = x_px as f64;
        let y = y_px as f64;
        let Some(byte) = self.hit_test_byte(x, y) else {
            return;
        };
        match click_count {
            2 => {
                self.drag_anchor = None;
                let s = self.buffer.to_text();
                let lo = editor_core::word_left(&s, byte.0);
                let hi = editor_core::word_right(&s, byte.0);
                self.selection = Selection { anchor: BytePos(lo), head: BytePos(hi) };
                self.cursor = Cursor::new(BytePos(hi));
            }
            3 => {
                self.drag_anchor = None;
                let Ok(lc) = self.buffer.byte_to_line_col(byte) else {
                    return;
                };
                let line = lc.line;
                let Ok(line_start) = self.buffer.line_to_byte(line) else {
                    return;
                };
                let Ok(line_len) = self.buffer.line_len_bytes(line) else {
                    return;
                };
                let end = line_start + line_len;
                self.selection = Selection { anchor: BytePos(line_start), head: BytePos(end) };
                self.cursor = Cursor::new(BytePos(end));
            }
            _ => {
                if shift {
                    if self.selection.is_empty() {
                        self.selection.anchor = self.cursor.pos();
                    }
                    // Keep anchor; move caret and head to click.
                    self.selection.head = byte;
                    self.cursor = Cursor::new(byte);
                    self.drag_anchor = Some(self.selection.anchor);
                } else {
                    self.drag_anchor = Some(byte);
                    self.cursor = Cursor::new(byte);
                    self.selection = Selection::empty(byte);
                }
            }
        }
        self.scroll_cursor_into_view();
    }

    fn apply_mouse_drag(&mut self, x_px: i32, y_px: i32) {
        if self.document_loading {
            return;
        }
        let y = y_px as f64;
        self.autoscroll_drag_edges(y);
        let Some(cb) = self.text_content_bottom_px() else {
            return;
        };
        let y_max = (cb - 1.0).max(0.0);
        let y_clamped = y.clamp(0.0, y_max);
        let Some(byte) = self.hit_test_byte(x_px as f64, y_clamped) else {
            return;
        };
        let anchor = self.drag_anchor.unwrap_or_else(|| self.cursor.pos());
        self.selection.anchor = anchor;
        self.selection.head = byte;
        self.cursor = Cursor::new(byte);
        self.scroll_cursor_into_view();
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
        self.drag_anchor = None;
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
            let selection_byte_range = if self.selection.is_empty() {
                None
            } else {
                let r = self.selection.range();
                Some((r.start.0, r.end.0))
            };
            let input = editor_render::FrameInput {
                buffer: &snap,
                scroll: self.scroll,
                clear_color: CLEAR,
                cursor_byte: self.cursor.pos().0.min(self.buffer.len_bytes()),
                cursor_blink_on: self.blink_on && self.selection.is_empty(),
                physical_size: physical,
                scale_factor: self.scale_factor,
                status: Some(status),
                dev_hud_line,
                selection_byte_range,
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
            EditorCommand::Copy => {
                self.clipboard_copy_selection();
                false
            }
            EditorCommand::Cut => {
                if !self.document_loading {
                    self.clipboard_cut();
                    self.scroll_cursor_into_view();
                }
                false
            }
            EditorCommand::Paste => {
                if !self.document_loading {
                    self.clipboard_paste();
                }
                false
            }
            EditorCommand::SelectAll => {
                self.select_all();
                false
            }
            EditorCommand::InsertText(text) => {
                self.insert_string(&text);
                false
            }
            EditorCommand::InsertNewline => {
                self.delete_selection_if_nonempty();
                let pos = self.cursor.pos().0;
                if let Ok(edit) = self
                    .buffer
                    .apply_edit(EditKind::Insert { pos: BytePos(pos), text: "\n".into() })
                {
                    self.undo.push(edit);
                    self.cursor = Cursor::new(BytePos(pos + 1));
                    self.selection = Selection::empty(self.cursor.pos());
                    self.dirty = true;
                    self.scroll_cursor_into_view();
                }
                false
            }
            EditorCommand::DeleteBackward => {
                if self.delete_selection_if_nonempty() {
                    self.scroll_cursor_into_view();
                    return false;
                }
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
                                self.collapse_selection_to_cursor();
                                self.dirty = true;
                                self.scroll_cursor_into_view();
                            }
                        }
                    }
                }
                false
            }
            EditorCommand::DeleteForward => {
                if self.delete_selection_if_nonempty() {
                    self.scroll_cursor_into_view();
                    return false;
                }
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
                                self.collapse_selection_to_cursor();
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
                    self.collapse_selection_to_cursor();
                    self.dirty = true;
                    self.scroll_cursor_into_view();
                }
                false
            }
            EditorCommand::Redo => {
                if let Ok(Some(_)) = self.undo.redo(&mut self.buffer) {
                    self.clamp_cursor_to_buffer();
                    self.collapse_selection_to_cursor();
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
            EditorCommand::ApplyCursorMotion { motion, extend_selection } => {
                if !extend_selection {
                    if self.cursor.apply(motion, &self.buffer).is_ok() {
                        self.collapse_selection_to_cursor();
                        self.scroll_cursor_into_view();
                    }
                } else {
                    if self.selection.is_empty() {
                        self.selection.anchor = self.cursor.pos();
                    }
                    if self.cursor.apply(motion, &self.buffer).is_ok() {
                        self.selection.head = self.cursor.pos();
                        self.scroll_cursor_into_view();
                    }
                }
                false
            }
            EditorCommand::DeleteWordBackward => {
                if self.delete_selection_if_nonempty() {
                    self.scroll_cursor_into_view();
                    return false;
                }
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
                        self.collapse_selection_to_cursor();
                        self.dirty = true;
                        self.scroll_cursor_into_view();
                    }
                }
                false
            }
            EditorCommand::DeleteWordForward => {
                if self.delete_selection_if_nonempty() {
                    self.scroll_cursor_into_view();
                    return false;
                }
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
                        self.collapse_selection_to_cursor();
                        self.dirty = true;
                        self.scroll_cursor_into_view();
                    }
                }
                false
            }
            EditorCommand::MouseClick { x_px, y_px, click_count, shift } => {
                self.apply_mouse_click(x_px, y_px, click_count, shift);
                false
            }
            EditorCommand::MouseDrag { x_px, y_px } => {
                self.apply_mouse_drag(x_px, y_px);
                false
            }
            EditorCommand::ScrollContent { delta_y_px } => {
                self.scroll.y_px -= delta_y_px;
                self.clamp_scroll();
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
            WindowEvent::CursorMoved { position, .. } => {
                self.last_pointer = position;
                if let Some(cmd) = self.mouse_chord.on_cursor_moved(position) {
                    let quit = self.apply_editor_command(cmd);
                    self.sync_window_title();
                    self.request_redraw();
                    if quit {
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    if let Some(cmd) = self.mouse_chord.on_left_button(
                        state,
                        button,
                        self.last_pointer,
                        self.modifiers,
                    ) {
                        let quit = self.apply_editor_command(cmd);
                        self.sync_window_title();
                        self.request_redraw();
                        if quit {
                            event_loop.exit();
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = scroll_delta_y_pixels(delta, self.scale_factor);
                if dy != 0.0 {
                    let quit =
                        self.apply_editor_command(EditorCommand::ScrollContent { delta_y_px: dy });
                    self.sync_window_title();
                    self.request_redraw();
                    if quit {
                        event_loop.exit();
                    }
                }
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

fn init_tracing(json_logs: bool) {
    let default_filter = if cfg!(debug_assertions) {
        "info,editor_app=info,editor_render=info,wgpu=warn"
    } else {
        "warn,editor_app=info,wgpu=warn"
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    #[cfg(feature = "tracy")]
    {
        let tracy = tracing_tracy::TracyLayer::default();
        if json_logs {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracy)
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_target(true)
                        .with_current_span(false),
                )
                .init();
        } else if cfg!(debug_assertions) {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracy)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true).pretty())
                .init();
        } else {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracy)
                .with(tracing_subscriber::fmt::layer().with_target(true).with_level(true).compact())
                .init();
        }
    }

    #[cfg(not(feature = "tracy"))]
    {
        if json_logs {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_target(true)
                        .with_current_span(false),
                )
                .init();
        } else if cfg!(debug_assertions) {
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
