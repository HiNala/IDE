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
mod perf_smoke;

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
use editor_git::GitRepo;
use editor_input::{map_key_event, scroll_delta_y_pixels, EditorCommand, MouseChordState};
use editor_io::{load_file_sync, save_file_sync, Encoding, LoadError, LoadedFile, SaveError};
use editor_settings::{LegacySessionMerge, LineEndingPreference, SettingsStore};
use editor_terminal::{detect_shell, Terminal, TerminalConfig, TerminalId};
use editor_ui::{
    paint_activity_bar, paint_tab_strip, ActivityIcon, ChromeQuad, CommandEntry, CommandPalette,
    FindBar, FrameChrome, QuickOpenPalette, Sidebar, TabHit, ACTIVITY_BAR_WIDTH, TAB_STRIP_HEIGHT,
};
use editor_workspace::entry::FileEntry;
use editor_workspace::{BufferId, BufferManager, FileSystemEvent, Workspace};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
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
use winit::window::{Fullscreen, Window, WindowId};

/// Crate / app version from `Cargo.toml`.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Dark background (`#1e1e1e`).
const CLEAR: wgpu::Color = wgpu::Color { r: 0.118, g: 0.118, b: 0.118, a: 1.0 };

#[derive(Debug)]
enum AppEvent {
    /// Toggle cursor blink phase (~2 Hz).
    BlinkTick,
    /// Drain PTY bytes into the emulator between frames.
    TerminalPump,
}

/// Files at or above this size are read on a background thread so the window can show immediately
/// (M06: avoid blocking the UI on huge reads).
const ASYNC_LOAD_MIN_BYTES: u64 = 4 * 1024 * 1024;

fn line_ending_label(le: LineEndingPreference) -> &'static str {
    match le {
        LineEndingPreference::Auto => "auto",
        LineEndingPreference::Lf => "lf",
        LineEndingPreference::Crlf => "crlf",
    }
}

/// Lines for the full-window settings overlay (M28); read-only until an in-app editor ships.
fn format_settings_overlay(store: &SettingsStore) -> Vec<String> {
    let path = editor_settings::paths::settings_file_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(no config path)".into());
    let s = store.settings();
    let active = match (&s.ai.active_provider, &s.ai.active_model) {
        (Some(p), Some(m)) => format!("{p} / {m}"),
        (Some(p), None) => format!("{p} (default model)"),
        (None, _) => "—".into(),
    };
    let mut lines = vec![
        "Settings".to_string(),
        String::new(),
        format!("File: {path}"),
        format!("Schema version: {}", s.version),
        String::new(),
        "AI".to_string(),
        format!("  Active: {active}"),
        format!("  Summarizer: {}", s.ai.enabled_summarizer),
        format!("  Vector index: {}", s.ai.enabled_vector_index),
        format!("  max_tokens_default: {}", s.ai.max_tokens_default),
        format!(
            "  temperature_default: {}",
            s.ai.temperature_default.map(|t| t.to_string()).unwrap_or_else(|| "—".into())
        ),
        String::new(),
        "Providers".to_string(),
    ];
    let mut keys: Vec<_> = s.ai.providers.keys().cloned().collect();
    keys.sort();
    for k in keys {
        let c = &s.ai.providers[&k];
        let bu = c.base_url.as_deref().unwrap_or("—");
        lines.push(format!("  {k}: enabled={} model={} base_url={bu}", c.enabled, c.default_model));
    }
    lines.push(String::new());
    lines.extend([
        "Editor".to_string(),
        format!("  font_size: {}", s.editor.font_size),
        format!("  line_ending: {}", line_ending_label(s.editor.line_ending)),
        format!(
            "  trim_trailing_whitespace_on_save: {}",
            s.editor.trim_trailing_whitespace_on_save
        ),
        format!("  ensure_newline_at_eof: {}", s.editor.ensure_newline_at_eof),
        format!("  word_wrap: {}", s.editor.word_wrap),
        format!("  undo_coalesce_ms: {}", s.editor.undo_coalesce_ms),
        String::new(),
        "Terminal".to_string(),
        format!(
            "  shell_override: {}",
            s.terminal
                .shell_override
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "—".into())
        ),
        format!("  font_size: {}", s.terminal.font_size),
        format!("  scrollback_lines: {}", s.terminal.scrollback_lines),
        format!("  default_height_pct: {}", s.terminal.default_height_pct),
        String::new(),
        "Skills".to_string(),
        format!("  disabled count: {}", s.skills.disabled.len()),
        format!("  extra_skill_dirs: {}", s.extra_skill_dirs.len()),
        String::new(),
        "Ctrl+, — toggle  ·  Esc — close".to_string(),
    ]);
    lines
}

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

    if args.first().is_some_and(|a| a == "index") {
        let rest: Vec<String> = args.into_iter().skip(1).collect();
        let root = match std::env::current_dir() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("editor-app: {e}");
                return Err(ExitCode::FAILURE);
            }
        };
        return match editor_index::cli::run_cli(&root, &rest) {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("{e}");
                Err(ExitCode::FAILURE)
            }
        };
    }

    for a in &args {
        if a == "--help" || a == "-h" {
            print_help();
            return Ok(());
        }
        if a.starts_with("--")
            && !matches!(a.as_str(), "--dry-run" | "--perf-smoke" | "--log-json" | "--dev-hud")
        {
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

    if args.iter().any(|a| a == "--perf-smoke") {
        if let Err(e) = perf_smoke::run() {
            eprintln!("{e:#}");
            return Err(ExitCode::FAILURE);
        }
        return Ok(());
    }

    let persisted = config::PersistedState::load();
    let open_arg = args.iter().find(|a| !a.starts_with('-')).cloned();
    let (file_arg, workspace_hint) = split_file_vs_folder_arg(open_arg);
    let plan = resolve_initial_plan(file_arg, &persisted);
    if let Err(e) = run_windowed(plan, persisted, start_dev_hud, workspace_hint) {
        eprintln!("{e:#}");
        return Err(ExitCode::FAILURE);
    }
    Ok(())
}

/// Splits the first positional CLI arg into (file-to-open, workspace-folder).
/// If the path is a directory → treat as workspace. If it's a regular file → open as file;
/// the parent folder is also suggested as the workspace root so the sidebar is useful.
fn split_file_vs_folder_arg(arg: Option<String>) -> (Option<String>, Option<PathBuf>) {
    let Some(s) = arg else {
        return (None, None);
    };
    let p = PathBuf::from(&s);
    if p.is_dir() {
        return (None, Some(p));
    }
    let folder = p.parent().filter(|d| d.is_dir()).map(Path::to_path_buf);
    (Some(s), folder)
}

fn print_help() {
    eprintln!(
        "\
editor-app — IDE binary (MVP in progress)

Usage:
  editor-app [path/to/file.txt] [--dry-run] [--perf-smoke] [--dev-hud] [--help]
  editor-app index [--rebuild | --status]

Arguments:
  path        Optional UTF-8 text file to open (falls back to bundled sample on error).

Commands:
  index       Local vector index over sidecars + code (see docs/VECTOR_INDEX.md). Uses cwd as workspace root.

Options:
  --dry-run   Headless GPU adapter/device init (no window).
  --perf-smoke  Scripted hidden-window frames + JSON line on stdout (PERF_SMOKE_* env vars).
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
    workspace_hint: Option<PathBuf>,
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
    let proxy_term = proxy.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(50));
        let _ = proxy_term.send_event(AppEvent::TerminalPump);
    });

    let sample = include_str!("../assets/sample.txt");
    let worker_pool = WorkerPool::new(None);
    let legacy = LegacySessionMerge::from_persisted(
        persisted.undo_coalesce_ms,
        &persisted.skills_disabled,
        &persisted.extra_skill_dirs,
    );
    let settings_store = SettingsStore::load_or_create(Some(&legacy));

    let app_sidebar_width = persisted.sidebar_width;
    let app_sidebar_visible = persisted.sidebar_visible.unwrap_or(false);

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
        settings_store,
        settings_overlay_lines: None,
        terminal_pane_visible: false,
        terminal_pane_fraction: 0.35,
        active_terminal_slot: 0,
        terminals: [None, None],
        terminal_next_id: 1,
        terminal_cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        terminal_focus: false,
        terminal_split_dragging: false,
        workspace: None,
        workspace_entries: Vec::new(),
        buffers: BufferManager::new(),
        active_buffer_id: None,
        sidebar: {
            let mut sb = Sidebar::new();
            if let Some(w) = app_sidebar_width {
                sb.width = w.max(120.0);
            }
            sb.visible = app_sidebar_visible;
            sb
        },
        quick_open: QuickOpenPalette::new(),
        command_palette: CommandPalette::new(),
        tab_hits: Vec::new(),
        find_bar: FindBar::default(),
        git_branch: None,
        git_last_refresh: Instant::now() - Duration::from_secs(10),
        gutter_marks: Vec::new(),
        gutter_marks_version: None,
        gutter_marks_for_path: None,
    };
    app.clamp_cursor_to_buffer();
    app.seed_initial_buffer_into_manager();
    if let Some(ws_root) = workspace_hint {
        app.open_workspace_folder(&ws_root);
    }
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
    settings_store: SettingsStore,
    /// Full-window settings overlay content (M28); `None` when hidden.
    settings_overlay_lines: Option<Vec<String>>,
    /// Whether the bottom integrated terminal strip is shown (**Ctrl+`**).
    terminal_pane_visible: bool,
    /// Share of window height (below status bar) used by the terminal when visible.
    terminal_pane_fraction: f32,
    active_terminal_slot: usize,
    terminals: [Option<Terminal>; 2],
    terminal_next_id: u64,
    /// Working directory for new PTY sessions.
    terminal_cwd: PathBuf,
    /// When true, typed keys go to the active PTY instead of the text buffer.
    terminal_focus: bool,
    /// User is dragging the editor/terminal split (resize handle).
    terminal_split_dragging: bool,
    // --- M13 workspace + multi-buffer ------------------------------------------------
    /// Project root when the user opened a folder (`Ctrl+K Ctrl+O` / dir CLI arg). `None` when
    /// only a single file is open.
    workspace: Option<Workspace>,
    /// Cached file entries from the most recent `walk_files`; used by the sidebar tree and
    /// the quick-open palette.
    workspace_entries: Vec<FileEntry>,
    /// Inactive buffers live here; the currently-edited buffer is mirrored into the top-level
    /// `buffer`/`cursor`/`selection`/... fields and is also tracked by `active_buffer_id`.
    buffers: BufferManager,
    /// Handle for the buffer whose state is currently mirrored into the App's active fields.
    /// `None` when the app is showing the bundled sample without a real buffer id yet.
    active_buffer_id: Option<BufferId>,
    // --- M14 chrome surfaces ---------------------------------------------------------
    sidebar: Sidebar,
    quick_open: QuickOpenPalette,
    /// Ctrl+Shift+P command palette (every `EditorCommand` discoverable).
    command_palette: CommandPalette,
    /// Tab hit boxes from the last frame's `paint_tab_strip` — used by mouse routing.
    tab_hits: Vec<TabHit>,
    // --- M16 in-buffer find / replace -----------------------------------------------
    /// Active find / replace overlay (M16). Hidden by default.
    find_bar: FindBar,
    // --- M18 light-touch git branch in the status bar --------------------------------
    git_branch: Option<String>,
    git_last_refresh: Instant,
    // --- M17/M18 diff gutter cache ---------------------------------------------------
    /// Per-line gutter marks for the active buffer (`None` slots = unchanged).
    /// Recomputed when the buffer text version advances past `gutter_marks_version`.
    gutter_marks: Vec<Option<editor_ui::GutterMark>>,
    /// Text-buffer version that produced `gutter_marks`. `None` means "never computed".
    gutter_marks_version: Option<u64>,
    /// Path whose HEAD blob seeded `gutter_marks` — when the active buffer
    /// changes to a different file we always recompute.
    gutter_marks_for_path: Option<PathBuf>,
}

impl App {
    /// Bottom status bar height in physical pixels (matches `editor-ui` / `TextLayer`).
    fn status_bar_height_px(&self) -> f32 {
        24.0 * self.scale_factor
    }

    /// Height reserved for the integrated terminal pane (physical px). `0` when hidden.
    fn terminal_pane_height_px(&self) -> f32 {
        if !self.terminal_pane_visible {
            return 0.0;
        }
        let Some(w) = self.window.as_ref() else {
            return 0.0;
        };
        let h = w.inner_size().height as f32;
        let status = self.status_bar_height_px();
        ((h - status) * self.terminal_pane_fraction).max(0.0)
    }

    /// Viewport height for the editor text canvas (window minus status bar and terminal pane).
    fn content_height_for_layout(&self) -> f32 {
        let Some(w) = self.window.as_ref() else {
            return 1.0;
        };
        (w.inner_size().height as f32
            - self.status_bar_height_px()
            - self.terminal_pane_height_px())
        .max(1.0)
    }

    fn active_terminal_slot_idx(&self) -> usize {
        let i = self.active_terminal_slot.min(1);
        if self.terminals[i].is_some() {
            i
        } else if self.terminals[0].is_some() {
            0
        } else {
            i
        }
    }

    fn terminal_cell_dimensions(&self) -> Option<(u16, u16, u16, u16)> {
        let renderer = self.renderer.as_ref()?;
        let w = self.window.as_ref()?;
        let line_h = renderer.line_height_px();
        let (_, char_w) = editor_render::compute_gutter_width_px(9_999, self.scale_factor);
        let (gutter_w, _) = editor_render::compute_gutter_width_px(9_999, self.scale_factor);
        let body_left = 8.0 + gutter_w;
        let pw = w.inner_size().width as f32;
        let term_h = self.terminal_pane_height_px();
        let cols = ((pw - body_left) / char_w.max(1e-6)).floor().max(1.0) as u16;
        let rows = (term_h / line_h).floor().max(1.0) as u16;
        let cw = char_w.round().max(1.0) as u16;
        let ch = line_h.round().max(1.0) as u16;
        Some((cols, rows, cw, ch))
    }

    fn sync_terminal_size(&mut self) {
        if !self.terminal_pane_visible {
            return;
        }
        let Some((cols, rows, cw, ch)) = self.terminal_cell_dimensions() else {
            return;
        };
        for slot in 0..2 {
            if let Some(ref mut t) = self.terminals[slot] {
                let _ = t.resize(cols, rows, cw, ch);
            }
        }
    }

    fn spawn_terminal(&mut self, slot: usize) -> Result<(), editor_terminal::TerminalError> {
        let (cols, rows, cw, ch) = match self.terminal_cell_dimensions() {
            Some(x) => x,
            None => {
                return Err(editor_terminal::TerminalError::Pty(
                    "terminal layout unavailable".into(),
                ));
            }
        };
        let id = TerminalId(self.terminal_next_id);
        self.terminal_next_id += 1;
        let shell = detect_shell(None)?;
        let term = Terminal::spawn(TerminalConfig {
            id,
            shell,
            cwd: self.terminal_cwd.clone(),
            cols,
            rows,
            cell_width_px: cw,
            cell_height_px: ch,
        })?;
        self.terminals[slot] = Some(term);
        Ok(())
    }

    fn ensure_terminal_spawned(&mut self) {
        if self.terminals[0].is_none() {
            if let Err(e) = self.spawn_terminal(0) {
                warn!(error = %e, "spawn integrated terminal");
            }
            self.active_terminal_slot = 0;
        }
    }

    /// Bottom Y of the editor text region in physical pixels (above terminal + status bar).
    fn editor_content_bottom_px(&self) -> Option<f64> {
        let w = self.window.as_ref()?;
        let h = w.inner_size().height as f64;
        let status = self.status_bar_height_px() as f64;
        let term = self.terminal_pane_height_px() as f64;
        Some(h - status - term)
    }

    fn pointer_in_terminal_pane(&self, y_px: f64) -> bool {
        if !self.terminal_pane_visible || self.terminal_pane_height_px() <= 0.5 {
            return false;
        }
        let Some(w) = self.window.as_ref() else {
            return false;
        };
        let h = w.inner_size().height as f64;
        let status = self.status_bar_height_px() as f64;
        let term = self.terminal_pane_height_px() as f64;
        y_px >= h - status - term && y_px < h - status
    }

    /// Y coordinate (physical px, top-left origin) of the editor/terminal divider.
    fn terminal_divider_top_px(&self) -> Option<f64> {
        if !self.terminal_pane_visible {
            return None;
        }
        let w = self.window.as_ref()?;
        let h = w.inner_size().height as f64;
        let status = self.status_bar_height_px() as f64;
        let term = self.terminal_pane_height_px() as f64;
        Some(h - status - term)
    }

    fn pointer_on_terminal_divider(&self, y_px: f64) -> bool {
        let Some(div) = self.terminal_divider_top_px() else {
            return false;
        };
        let slop = (4.0_f64 * f64::from(self.scale_factor)).max(2.0);
        (y_px - div).abs() <= slop
    }

    fn update_terminal_split_from_pointer_y(&mut self, y_px: f64) {
        let Some(w) = self.window.as_ref() else {
            return;
        };
        let h = w.inner_size().height as f64;
        let status = self.status_bar_height_px() as f64;
        let inner = h - status;
        if inner <= 96.0 {
            return;
        }
        let term_h = (h - status - y_px).clamp(48.0, inner - 48.0);
        self.terminal_pane_fraction = (term_h / inner) as f32;
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

    /// Register the initial (post-CLI) buffer with the `BufferManager` so tab strip
    /// and buffer cycling can see it. Called once after the App struct is built.
    fn seed_initial_buffer_into_manager(&mut self) {
        if self.active_buffer_id.is_some() {
            return;
        }
        // Clone the initial buffer contents into a BufferState stored in the manager.
        // The live App fields remain the authoritative mirror for the active slot.
        let id = self.buffers.create_empty();
        self.sync_active_to_manager_with_id(id);
        self.active_buffer_id = Some(id);
    }

    /// Copy the current App editor state into the specified buffer slot in the manager.
    /// Used before switching away from the active buffer and whenever we need the
    /// manager-side state to be in sync (tab strip rendering, external polling).
    fn sync_active_to_manager_with_id(&mut self, id: BufferId) {
        let Some(st) = self.buffers.get_mut(id) else {
            return;
        };
        st.buffer = self.buffer.clone();
        st.cursor = self.cursor;
        st.selection = self.selection;
        // UndoStack has no Clone; recreate empty — multi-session undo across switches
        // is out of scope for this wiring pass (noted in FOLLOWUPS).
        st.undo = UndoStack::default();
        st.scroll = self.scroll;
        st.path = self.open_path.clone();
        st.disk_encoding = self.disk_encoding;
        st.dirty = self.dirty;
        st.external_modified = self.external_modified;
        st.file_mtime = self.file_mtime;
    }

    /// Snapshot-style variant for reads (doesn't touch undo): the sync writes the latest
    /// live fields into the backing `BufferState` without creating a fresh UndoStack.
    fn sync_active_metadata_only(&mut self) {
        let Some(id) = self.active_buffer_id else {
            return;
        };
        let Some(st) = self.buffers.get_mut(id) else {
            return;
        };
        st.buffer = self.buffer.clone();
        st.cursor = self.cursor;
        st.selection = self.selection;
        st.scroll = self.scroll;
        st.path = self.open_path.clone();
        st.disk_encoding = self.disk_encoding;
        st.dirty = self.dirty;
        st.external_modified = self.external_modified;
        st.file_mtime = self.file_mtime;
    }

    /// Load the fields of `BufferState` into the App's active slot. Caller must have
    /// already saved the previous active state (see [`Self::sync_active_to_manager_with_id`]).
    fn load_state_from_buffer(&mut self, id: BufferId) {
        let Some(st) = self.buffers.get(id) else {
            return;
        };
        self.buffer = st.buffer.clone();
        self.cursor = st.cursor;
        self.selection = st.selection;
        self.undo = UndoStack::default();
        self.scroll = st.scroll;
        self.open_path = st.path.clone();
        self.disk_encoding = st.disk_encoding;
        self.dirty = st.dirty;
        self.external_modified = st.external_modified;
        self.file_mtime = st.file_mtime;
        self.active_buffer_id = Some(id);
        self.drag_anchor = None;
        self.clamp_cursor_to_buffer();
    }

    /// Switch to the given buffer, saving the current active state first.
    fn switch_to_buffer(&mut self, id: BufferId) {
        if self.active_buffer_id == Some(id) {
            return;
        }
        if let Some(cur) = self.active_buffer_id {
            self.sync_active_to_manager_with_id(cur);
        }
        let _ = self.buffers.switch_to(id);
        self.load_state_from_buffer(id);
        self.reveal_active_in_sidebar();
        self.sync_window_title();
    }

    fn workspace_root(&self) -> Option<&Path> {
        self.workspace.as_ref().map(|w| w.root())
    }

    /// If the active file lives under the workspace root, expand ancestors so the sidebar
    /// shows it (keyboard focus unchanged). No-op outside a workspace.
    fn reveal_active_in_sidebar(&mut self) {
        let (Some(abs), Some(root)) = (self.open_path.as_ref(), self.workspace_root()) else {
            return;
        };
        let Ok(rel) = abs.strip_prefix(root) else {
            return;
        };
        self.sidebar.reveal_path(rel);
    }

    /// Open (or switch to) `path` as a new tab; mirrors state into the App's active slot.
    fn open_path_as_buffer(&mut self, path: &Path) {
        if let Some(cur) = self.active_buffer_id {
            self.sync_active_to_manager_with_id(cur);
        }
        match self.buffers.open_file_coalesced(path, self.persisted.undo_coalesce_ms) {
            Ok(id) => {
                self.load_state_from_buffer(id);
                self.reveal_active_in_sidebar();
                self.sync_window_title();
            }
            Err(e) => warn!(path = %path.display(), error = %e, "open_path_as_buffer: load failed"),
        }
    }

    fn open_workspace_folder(&mut self, root: &Path) {
        match Workspace::open(root) {
            Ok(ws) => {
                info!(root = %ws.root().display(), "workspace opened");
                self.workspace = Some(ws);
                self.rebuild_workspace_entries();
                self.refresh_git_branch(true);
                // Auto-show the sidebar so folder opens are immediately useful.
                if !self.sidebar.visible {
                    self.sidebar.visible = true;
                }
                self.reveal_active_in_sidebar();
            }
            Err(e) => warn!(error = %e, root = %root.display(), "workspace open failed"),
        }
    }

    fn rebuild_workspace_entries(&mut self) {
        let Some(ws) = self.workspace.as_ref() else {
            self.workspace_entries.clear();
            self.sidebar.rebuild_flat(&self.workspace_entries);
            self.quick_open_refresh();
            return;
        };
        match ws.walk_files() {
            Ok(entries) => {
                self.workspace_entries = entries;
                self.sidebar.rebuild_flat(&self.workspace_entries);
                self.quick_open_refresh();
            }
            Err(e) => warn!(error = %e, "workspace walk failed"),
        }
    }

    fn quick_open_refresh(&mut self) {
        let Some(ws) = self.workspace.as_ref() else {
            return;
        };
        self.quick_open.set_workspace_files(ws, &self.workspace_entries);
    }

    fn refresh_git_branch(&mut self, force: bool) {
        let due = force || self.git_last_refresh.elapsed() >= Duration::from_secs(5);
        if !due {
            return;
        }
        self.git_last_refresh = Instant::now();
        let start = self
            .workspace
            .as_ref()
            .map(|w| w.root().to_path_buf())
            .or_else(|| self.open_path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        match GitRepo::discover(&start) {
            Ok(Some(repo)) => {
                self.git_branch = repo.branch_name();
            }
            Ok(None) => self.git_branch = None,
            Err(e) => {
                debug!(error = %e, "git discover failed");
                self.git_branch = None;
            }
        }
    }

    fn apply_workspace_event(&mut self, ev: FileSystemEvent) {
        match ev {
            FileSystemEvent::Created(_) | FileSystemEvent::Removed(_) => {
                // Tree changed — rebuild sidebar + quick-open.
                self.rebuild_workspace_entries();
            }
            FileSystemEvent::Renamed { from, to } => {
                self.buffers.rename_buffer_path(&from, &to);
                if self
                    .active_buffer_id
                    .and_then(|id| self.buffers.get(id))
                    .and_then(|s| s.path.as_ref())
                    .is_some_and(|p| p == &to)
                {
                    self.open_path = Some(to.clone());
                }
                self.rebuild_workspace_entries();
            }
            FileSystemEvent::Modified(path) => {
                // Flag any matching buffer (including the active mirror) as externally modified.
                let matches_active =
                    self.open_path.as_ref().is_some_and(|cur| BufferManager::same_path(cur, &path));
                if matches_active {
                    if let Ok(m) = std::fs::metadata(&path).and_then(|x| x.modified()) {
                        if self.file_mtime.is_none_or(|prev| prev != m) {
                            self.external_modified = true;
                        }
                    }
                }
                if let Some(id) = self.buffers.find_by_path(&path) {
                    if let Some(st) = self.buffers.get_mut(id) {
                        if let Ok(m) = std::fs::metadata(&path).and_then(|x| x.modified()) {
                            if st.file_mtime.is_none_or(|prev| prev != m) {
                                st.external_modified = true;
                            }
                        }
                    }
                }
            }
        }
    }

    fn poll_workspace_events(&mut self) {
        let Some(ws) = self.workspace.as_ref() else {
            return;
        };
        let events = ws.poll_events();
        if events.is_empty() {
            return;
        }
        for ev in events {
            self.apply_workspace_event(ev);
        }
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

    /// Scroll by one line when the pointer sits in the top/bottom margin during drag (M09 §6.8).
    fn autoscroll_drag_edges(&mut self, y_px: f64) {
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let Some(content_bottom) = self.editor_content_bottom_px() else {
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

    /// Open the find bar with optional replace row; seed query from the current selection
    /// if it is a single-line slice.
    fn open_find_bar(&mut self, with_replace: bool) {
        self.find_bar.visible = true;
        self.find_bar.replace_row_visible = with_replace;
        self.find_bar.focus_replace = false;
        if !self.selection.is_empty() {
            let r = self.selection.range();
            if let Ok(text) = self.buffer.slice_to_string(r.start..r.end) {
                if !text.contains('\n') && text.len() <= 256 {
                    self.find_bar.query = text;
                    self.find_bar.query_cursor = self.find_bar.query.len();
                }
            }
        }
        // Recompute matches right away.
        self.find_bar.rerun_search(&self.buffer.snapshot());
        // If a match exists, pick the one closest to the caret.
        if !self.find_bar.matches.is_empty() {
            let caret = self.cursor.pos().0;
            let idx =
                self.find_bar.matches.iter().position(|m| m.byte_range.start >= caret).unwrap_or(0);
            self.find_bar.current_match = Some(idx.min(self.find_bar.matches.len() - 1));
        }
        self.reveal_current_match();
        self.request_redraw();
    }

    /// Move caret + selection to the current match (if any) and scroll it into view.
    fn reveal_current_match(&mut self) {
        let Some(idx) = self.find_bar.current_match else {
            return;
        };
        let Some(m) = self.find_bar.matches.get(idx) else {
            return;
        };
        let start = BytePos(m.byte_range.start);
        let end = BytePos(m.byte_range.end);
        self.selection = Selection { anchor: start, head: end };
        self.cursor = Cursor::new(end);
        self.scroll_cursor_into_view();
    }

    /// Replace the current match with `find_bar.replace`, advance to the next match. Returns
    /// whether an edit occurred.
    fn apply_replace_current(&mut self) -> bool {
        if self.find_bar.current_match.is_none() || self.find_bar.matches.is_empty() {
            return false;
        }
        let idx = self.find_bar.current_match.unwrap();
        let replacement = self.find_bar.replace.clone();
        match editor_search::replace_one(
            &mut self.buffer,
            idx,
            &self.find_bar.matches,
            &replacement,
        ) {
            Ok((edits, _delta)) => {
                for e in edits {
                    self.undo.push(e);
                }
                self.dirty = true;
                self.find_bar.rerun_search(&self.buffer.snapshot());
                // Advance to next if any remain.
                if !self.find_bar.matches.is_empty() {
                    let next = idx.min(self.find_bar.matches.len() - 1);
                    self.find_bar.current_match = Some(next);
                    self.reveal_current_match();
                } else {
                    self.find_bar.current_match = None;
                }
                true
            }
            Err(e) => {
                warn!(error = %e, "find: replace_one failed");
                false
            }
        }
    }

    /// Replace every match. Returns the number of replacements applied.
    fn apply_replace_all(&mut self) -> usize {
        if self.find_bar.matches.is_empty() {
            return 0;
        }
        let replacement = self.find_bar.replace.clone();
        match editor_search::replace_all(&mut self.buffer, &self.find_bar.matches, &replacement) {
            Ok((edits, count)) => {
                for e in edits {
                    self.undo.push(e);
                }
                if count > 0 {
                    self.dirty = true;
                }
                self.find_bar.rerun_search(&self.buffer.snapshot());
                count
            }
            Err(e) => {
                warn!(error = %e, "find: replace_all failed");
                0
            }
        }
    }

    /// Handle a key press when the find bar is active. Returns `true` if consumed.
    fn handle_find_bar_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        use winit::keyboard::{KeyCode, PhysicalKey};
        let PhysicalKey::Code(code) = event.physical_key else {
            return false;
        };
        let shift = self.modifiers.shift_key();
        let ctrl = self.modifiers.control_key();
        match code {
            KeyCode::Escape => {
                self.find_bar.visible = false;
                self.find_bar.matches.clear();
                self.find_bar.current_match = None;
                self.find_bar.regex_error = None;
                true
            }
            KeyCode::Enter => {
                if ctrl && self.find_bar.replace_row_visible {
                    let _ = self.apply_replace_all();
                } else if self.find_bar.focus_replace && self.find_bar.replace_row_visible {
                    self.apply_replace_current();
                } else if shift {
                    self.find_bar.prev_match();
                    self.reveal_current_match();
                } else {
                    self.find_bar.next_match();
                    self.reveal_current_match();
                }
                true
            }
            KeyCode::F3 => {
                if shift {
                    self.find_bar.prev_match();
                } else {
                    self.find_bar.next_match();
                }
                self.reveal_current_match();
                true
            }
            KeyCode::Tab => {
                if self.find_bar.replace_row_visible {
                    self.find_bar.focus_replace = !self.find_bar.focus_replace;
                }
                true
            }
            KeyCode::Backspace => {
                let (field, cursor) = if self.find_bar.focus_replace {
                    (&mut self.find_bar.replace, &mut self.find_bar.replace_cursor)
                } else {
                    (&mut self.find_bar.query, &mut self.find_bar.query_cursor)
                };
                if *cursor > 0 {
                    let mut new_cursor = *cursor - 1;
                    while new_cursor > 0 && !field.is_char_boundary(new_cursor) {
                        new_cursor -= 1;
                    }
                    field.replace_range(new_cursor..*cursor, "");
                    *cursor = new_cursor;
                }
                if !self.find_bar.focus_replace {
                    self.find_bar.rerun_search(&self.buffer.snapshot());
                }
                true
            }
            KeyCode::ArrowLeft => {
                let (field, cursor) = if self.find_bar.focus_replace {
                    (&self.find_bar.replace, &mut self.find_bar.replace_cursor)
                } else {
                    (&self.find_bar.query, &mut self.find_bar.query_cursor)
                };
                if *cursor > 0 {
                    let mut new_cursor = *cursor - 1;
                    while new_cursor > 0 && !field.is_char_boundary(new_cursor) {
                        new_cursor -= 1;
                    }
                    *cursor = new_cursor;
                }
                true
            }
            KeyCode::ArrowRight => {
                let (field, cursor) = if self.find_bar.focus_replace {
                    (&self.find_bar.replace, &mut self.find_bar.replace_cursor)
                } else {
                    (&self.find_bar.query, &mut self.find_bar.query_cursor)
                };
                let len = field.len();
                if *cursor < len {
                    let mut new_cursor = *cursor + 1;
                    while new_cursor < len && !field.is_char_boundary(new_cursor) {
                        new_cursor += 1;
                    }
                    *cursor = new_cursor;
                }
                true
            }
            _ => {
                if let Some(t) = event.text.as_ref() {
                    if !t.is_empty() && t.chars().all(|c| !c.is_control()) {
                        let (field, cursor) = if self.find_bar.focus_replace {
                            (&mut self.find_bar.replace, &mut self.find_bar.replace_cursor)
                        } else {
                            (&mut self.find_bar.query, &mut self.find_bar.query_cursor)
                        };
                        field.insert_str(*cursor, t.as_str());
                        *cursor += t.len();
                        if !self.find_bar.focus_replace {
                            self.find_bar.rerun_search(&self.buffer.snapshot());
                        }
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Handle a key press when the sidebar has keyboard focus. Returns `true`
    /// when the sidebar consumed the event (caller should not dispatch further).
    ///
    /// Keys: **↑/↓** move the highlight; **←** collapses or walks to the parent;
    /// **→** expands a directory; **Enter / Space** opens a file or toggles a
    /// directory; **Home/End** jump to first/last row; **PageUp/PageDown** step
    /// by a viewport; **Esc** defocuses back into the editor.
    fn handle_sidebar_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        use winit::keyboard::{KeyCode, PhysicalKey};
        if !self.sidebar.focused || !self.sidebar.visible {
            return false;
        }
        let PhysicalKey::Code(code) = event.physical_key else {
            return false;
        };
        // A rough "rows per page" estimate for PageUp/PageDown.
        let page = {
            let row_h = editor_ui::sidebar::ROW_LINE_HEIGHT.max(1.0);
            let header_h = editor_ui::sidebar::HEADER_HEIGHT;
            let status_h = self.status_bar_height_px() / self.scale_factor;
            let term_h = self.terminal_pane_height_px() / self.scale_factor;
            let h_logical = self
                .window
                .as_ref()
                .map(|w| {
                    w.inner_size().height as f32 / self.scale_factor - header_h - status_h - term_h
                })
                .unwrap_or(240.0)
                .max(row_h * 4.0);
            (h_logical / row_h).floor().max(1.0) as isize
        };
        match code {
            KeyCode::Escape => {
                self.sidebar.focused = false;
                self.request_redraw();
                true
            }
            KeyCode::ArrowUp => {
                self.sidebar.move_highlight(-1);
                self.request_redraw();
                true
            }
            KeyCode::ArrowDown => {
                self.sidebar.move_highlight(1);
                self.request_redraw();
                true
            }
            KeyCode::PageUp => {
                self.sidebar.move_highlight(-page);
                self.request_redraw();
                true
            }
            KeyCode::PageDown => {
                self.sidebar.move_highlight(page);
                self.request_redraw();
                true
            }
            KeyCode::Home => {
                self.sidebar.highlight_first();
                self.request_redraw();
                true
            }
            KeyCode::End => {
                self.sidebar.highlight_last();
                self.request_redraw();
                true
            }
            KeyCode::ArrowRight => {
                if self.sidebar.expand_highlighted() {
                    self.sidebar.rebuild_flat(&self.workspace_entries);
                }
                self.request_redraw();
                true
            }
            KeyCode::ArrowLeft => {
                self.sidebar.collapse_or_parent();
                self.sidebar.rebuild_flat(&self.workspace_entries);
                self.request_redraw();
                true
            }
            KeyCode::Enter | KeyCode::Space => {
                self.activate_highlighted_sidebar_row();
                self.request_redraw();
                true
            }
            _ => {
                // Swallow plain typing so it doesn't leak into the active buffer.
                // Ctrl/Alt/Super combos still fall through so global shortcuts work.
                let m = self.modifiers;
                !(m.control_key() || m.alt_key() || m.super_key())
            }
        }
    }

    /// Enter/Space on a directory toggles it; on a file, opens it in a new tab.
    fn activate_highlighted_sidebar_row(&mut self) {
        let Some(rel) = self.sidebar.highlighted.clone() else { return };
        let Some(row) = self.sidebar.flat_rows().iter().find(|r| r.rel == rel).cloned() else {
            return;
        };
        if row.is_dir {
            self.sidebar.toggle_dir(&row.rel);
            self.sidebar.rebuild_flat(&self.workspace_entries);
        } else if let Some(root) = self.workspace_root() {
            let abs = root.join(&row.rel);
            self.open_path_as_buffer(&abs);
        }
    }

    /// Seed the command palette with the app's inventory of `EditorCommand`s
    /// the first time the palette is opened. Idempotent: subsequent calls do
    /// nothing because the entry list never changes at runtime.
    fn ensure_command_palette_seeded(&mut self) {
        if !self.command_palette.is_empty() {
            return;
        }
        // (id, title, keybinding-hint). `id` must match the branch in
        // `apply_command_palette_selection` below.
        const ENTRIES: &[(&str, &str, Option<&str>)] = &[
            ("file.new", "File: New Buffer", Some("Ctrl+N")),
            ("file.open", "File: Open...", Some("Ctrl+O")),
            ("file.save", "File: Save", Some("Ctrl+S")),
            ("file.close", "File: Close Buffer", Some("Ctrl+W")),
            ("edit.undo", "Edit: Undo", Some("Ctrl+Z")),
            ("edit.redo", "Edit: Redo", Some("Ctrl+Y")),
            ("edit.cut", "Edit: Cut", Some("Ctrl+X")),
            ("edit.copy", "Edit: Copy", Some("Ctrl+C")),
            ("edit.paste", "Edit: Paste", Some("Ctrl+V")),
            ("edit.select_all", "Edit: Select All", Some("Ctrl+A")),
            ("edit.find", "Edit: Find in File", Some("Ctrl+F")),
            ("edit.replace", "Edit: Replace in File", Some("Ctrl+H")),
            ("edit.find_next", "Edit: Find Next", Some("F3")),
            ("edit.find_prev", "Edit: Find Previous", Some("Shift+F3")),
            ("view.sidebar", "View: Toggle Sidebar", Some("Ctrl+B")),
            ("view.focus_sidebar", "View: Focus Sidebar", Some("Ctrl+Shift+E")),
            ("view.quick_open", "Go: Quick Open", Some("Ctrl+P")),
            ("view.command_palette", "Show Command Palette", Some("Ctrl+Shift+P")),
            ("view.fullscreen", "View: Toggle Fullscreen", Some("F11")),
            ("view.dev_hud", "View: Toggle Developer HUD", Some("Ctrl+F11")),
            ("view.terminal", "View: Toggle Terminal", Some("Ctrl+`")),
            ("view.terminal_new", "Terminal: New Session", Some("Ctrl+Shift+`")),
            ("buffer.next", "Buffer: Next", Some("Ctrl+Tab")),
            ("buffer.prev", "Buffer: Previous", Some("Ctrl+Shift+Tab")),
            ("pref.settings", "Preferences: Open Settings", Some("Ctrl+,")),
            ("app.quit", "Quit", Some("Ctrl+Q")),
        ];
        let entries: Vec<CommandEntry> = ENTRIES
            .iter()
            .map(|(id, title, hint)| CommandEntry {
                id,
                title: (*title).to_string(),
                hint: hint.map(|s| s.to_string()),
            })
            .collect();
        self.command_palette.set_entries(entries);
    }

    /// Dispatch the currently highlighted command from the palette and hide it.
    /// Returns `true` when something was dispatched (caller should skip other handlers).
    fn apply_command_palette_selection(&mut self) -> bool {
        let Some(id) = self.command_palette.selected_id() else {
            self.command_palette.hide();
            return true;
        };
        self.command_palette.hide();
        let cmd = match id {
            "file.new" => EditorCommand::NewBuffer,
            "file.open" => EditorCommand::Open,
            "file.save" => EditorCommand::Save,
            "file.close" => EditorCommand::CloseBuffer,
            "edit.undo" => EditorCommand::Undo,
            "edit.redo" => EditorCommand::Redo,
            "edit.cut" => EditorCommand::Cut,
            "edit.copy" => EditorCommand::Copy,
            "edit.paste" => EditorCommand::Paste,
            "edit.select_all" => EditorCommand::SelectAll,
            "edit.find" => EditorCommand::FindInFile,
            "edit.replace" => EditorCommand::ReplaceInFile,
            "edit.find_next" => EditorCommand::FindNext,
            "edit.find_prev" => EditorCommand::FindPrev,
            "view.sidebar" => EditorCommand::ToggleSidebar,
            "view.focus_sidebar" => EditorCommand::FocusSidebar,
            "view.quick_open" => EditorCommand::ToggleQuickOpen,
            "view.command_palette" => EditorCommand::OpenCommandPalette,
            "view.fullscreen" => EditorCommand::ToggleFullscreen,
            "view.dev_hud" => EditorCommand::ToggleDevHud,
            "view.terminal" => EditorCommand::ToggleTerminalPane,
            "view.terminal_new" => EditorCommand::NewIntegratedTerminal,
            "buffer.next" => EditorCommand::NextBuffer,
            "buffer.prev" => EditorCommand::PrevBuffer,
            "pref.settings" => EditorCommand::OpenSettings,
            "app.quit" => EditorCommand::Quit,
            other => {
                warn!("command palette: unknown id {other:?}");
                return true;
            }
        };
        let _ = self.apply_editor_command(cmd);
        true
    }

    /// Handle a key press while the command palette is visible. Mirrors the
    /// quick-open palette conventions (Enter selects, Esc dismisses, arrows
    /// move, printable chars filter, Backspace pops).
    fn handle_command_palette_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        use winit::keyboard::{KeyCode, PhysicalKey};
        let PhysicalKey::Code(code) = event.physical_key else {
            return false;
        };
        match code {
            KeyCode::Escape => {
                self.command_palette.hide();
                true
            }
            KeyCode::Enter => self.apply_command_palette_selection(),
            KeyCode::ArrowUp => {
                self.command_palette.move_selection(-1);
                true
            }
            KeyCode::ArrowDown => {
                self.command_palette.move_selection(1);
                true
            }
            KeyCode::Backspace => {
                self.command_palette.backspace();
                true
            }
            _ => {
                if let Some(t) = event.text.as_ref() {
                    if !t.is_empty() && t.chars().all(|c| !c.is_control()) {
                        for ch in t.chars() {
                            self.command_palette.push_char(ch);
                        }
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Handle a key press when the quick-open palette is active.
    /// Returns `true` when the palette consumed the event (caller should not dispatch further).
    fn handle_quick_open_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        use winit::keyboard::{KeyCode, PhysicalKey};
        let PhysicalKey::Code(code) = event.physical_key else {
            return false;
        };
        match code {
            KeyCode::Escape => {
                self.quick_open.hide();
                true
            }
            KeyCode::Enter => {
                if let Some(root) = self.workspace_root().map(Path::to_path_buf) {
                    if let Some(path) = self.quick_open.selected_absolute(&root) {
                        self.quick_open.hide();
                        self.open_path_as_buffer(&path);
                        return true;
                    }
                }
                self.quick_open.hide();
                true
            }
            KeyCode::ArrowUp => {
                self.quick_open.move_selection(-1);
                true
            }
            KeyCode::ArrowDown => {
                self.quick_open.move_selection(1);
                true
            }
            KeyCode::Backspace => {
                self.quick_open.backspace();
                true
            }
            _ => {
                // Accept printable chars from `KeyEvent::text` only; filter out control-only.
                if let Some(t) = event.text.as_ref() {
                    if !t.is_empty() && t.chars().all(|c| !c.is_control()) {
                        for ch in t.chars() {
                            self.quick_open.push_char(ch);
                        }
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Left chrome width in physical px: activity bar + sidebar (if visible).
    /// Used by mouse routing and hit-testing so clicks on chrome don't fall through to the editor.
    fn left_chrome_width_px(&self) -> f32 {
        let activity = ACTIVITY_BAR_WIDTH * self.scale_factor;
        let sidebar =
            if self.sidebar.visible { self.sidebar.width * self.scale_factor } else { 0.0 };
        activity + sidebar
    }

    /// Just the activity bar width (always painted) in physical px.
    fn activity_bar_width_px(&self) -> f32 {
        ACTIVITY_BAR_WIDTH * self.scale_factor
    }

    /// Tab strip height in physical px when any buffer is tracked, else 0.
    fn tabstrip_height_px(&self) -> f32 {
        if !self.buffers.is_empty() {
            TAB_STRIP_HEIGHT * self.scale_factor
        } else {
            0.0
        }
    }

    /// Breadcrumbs strip height in physical px — painted when a tab strip is
    /// visible AND the active buffer has a displayable path.
    fn breadcrumbs_height_px(&self) -> f32 {
        if self.buffers.is_empty() || self.active_path_rel().is_none() {
            0.0
        } else {
            editor_ui::BREADCRUMBS_HEIGHT * self.scale_factor
        }
    }

    /// Total height of top chrome (tabstrip + breadcrumbs) — used by mouse
    /// hit-testing to offset the editor canvas.
    fn top_chrome_height_px(&self) -> f32 {
        self.tabstrip_height_px() + self.breadcrumbs_height_px()
    }

    /// Workspace-relative path of the active buffer, or its `file_name()` when
    /// no workspace is open, or `None` for untitled buffers.
    fn active_path_rel(&self) -> Option<PathBuf> {
        let abs = self.open_path.as_ref()?;
        if let Some(root) = self.workspace_root() {
            if let Ok(rel) = abs.strip_prefix(root) {
                return Some(rel.to_path_buf());
            }
        }
        abs.file_name().map(PathBuf::from)
    }

    /// Recompute the per-line diff-vs-HEAD gutter marks **when** the active
    /// buffer's text has advanced past the cached version. Cheap no-op in the
    /// common "no edit this frame" case.
    ///
    /// Failures (no git repo, path not in HEAD, read errors) silently drop
    /// the cache; the painter will simply skip drawing stripes.
    fn refresh_gutter_marks_if_stale(&mut self) {
        let version = self.buffer.version();
        let total_lines = self.buffer.len_lines().max(1);
        let path_matches = self.gutter_marks_for_path.as_deref() == self.open_path.as_deref();
        if path_matches && self.gutter_marks_version == Some(version) {
            return;
        }
        let Some(abs) = self.open_path.clone() else {
            self.gutter_marks.clear();
            self.gutter_marks_version = Some(version);
            self.gutter_marks_for_path = None;
            return;
        };
        let repo_root = self
            .workspace_root()
            .map(Path::to_path_buf)
            .or_else(|| abs.parent().map(Path::to_path_buf));
        let Some(start) = repo_root else {
            self.gutter_marks.clear();
            self.gutter_marks_version = Some(version);
            self.gutter_marks_for_path = Some(abs);
            return;
        };
        let repo = match GitRepo::discover(&start) {
            Ok(Some(r)) => r,
            _ => {
                self.gutter_marks.clear();
                self.gutter_marks_version = Some(version);
                self.gutter_marks_for_path = Some(abs);
                return;
            }
        };
        // Use the gix-reported workdir so we compute the proper relative path
        // even when the app's workspace root sits at a subdirectory of the repo.
        let rel = match abs.strip_prefix(repo.workdir()) {
            Ok(r) => r.to_path_buf(),
            Err(_) => {
                self.gutter_marks.clear();
                self.gutter_marks_version = Some(version);
                self.gutter_marks_for_path = Some(abs);
                return;
            }
        };
        let text = self.buffer.to_text();
        let hunks = match repo.line_diff_vs_head(&rel, &text) {
            Ok(h) => h,
            Err(e) => {
                debug!(?rel, error = %e, "diff-vs-HEAD failed; clearing gutter marks");
                Vec::new()
            }
        };
        self.gutter_marks = editor_ui::compute_gutter_marks(&hunks, total_lines);
        self.gutter_marks_version = Some(version);
        self.gutter_marks_for_path = Some(abs);
    }

    /// Map physical window pixel to a UTF-8 boundary byte offset (M09; matches `editor-render` layout).
    fn hit_test_byte(&self, x_px: f64, y_px: f64) -> Option<BytePos> {
        let renderer = self.renderer.as_ref()?;
        let w = self.window.as_ref()?;
        let physical = w.inner_size();
        let line_h = renderer.line_height_px();
        let status_h = self.status_bar_height_px();
        let term_h = self.terminal_pane_height_px();
        let chrome_left_w = self.left_chrome_width_px();
        let top_h = self.top_chrome_height_px();
        let (gutter_w, char_w) =
            editor_render::compute_gutter_width_px(self.buffer.len_lines(), self.scale_factor);
        let content_bottom = physical.height as f32 - status_h - term_h;
        if y_px < top_h as f64 || y_px >= content_bottom as f64 {
            return None;
        }
        if x_px < chrome_left_w as f64 {
            return None;
        }
        let total_lines = self.buffer.len_lines();
        if total_lines == 0 {
            return Some(BytePos(0));
        }
        let y_rel = y_px as f32 - top_h;
        let line_idx_f = (y_rel - 4.0 + self.scroll.y_px) / line_h;
        let mut line_idx = line_idx_f.floor() as isize;
        if line_idx < 0 {
            line_idx = 0;
        }
        let mut line_idx = line_idx as usize;
        if line_idx >= total_lines {
            line_idx = total_lines.saturating_sub(1);
        }
        let body_left = chrome_left_w + 8.0 + gutter_w;
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

        // Quick-open overlay: outside-click dismisses; inside is swallowed (Enter/Esc handle selection).
        if self.quick_open.visible {
            self.quick_open.hide();
            self.request_redraw();
            return;
        }
        if self.command_palette.visible {
            self.command_palette.hide();
            self.request_redraw();
            return;
        }

        // Activity bar clicks (the leftmost slim column). Today: the first icon toggles
        // the sidebar; others are placeholders.
        let activity_w = self.activity_bar_width_px() as f64;
        if x < activity_w {
            let slot_h = (editor_ui::activity_bar::ACTIVITY_ICON_HEIGHT * self.scale_factor) as f64;
            let slot = (y / slot_h.max(1.0)).floor() as i32;
            if slot == 0 {
                let _ = self.apply_editor_command(EditorCommand::ToggleSidebar);
            }
            self.request_redraw();
            return;
        }

        // Sidebar clicks (right of the activity bar, when visible).
        let chrome_left_w = self.left_chrome_width_px() as f64;
        if self.sidebar.visible && x < chrome_left_w {
            // A click anywhere in the sidebar column gives it keyboard focus.
            self.sidebar.focused = true;
            self.terminal_focus = false;
            // Subtract the header so the row index calculation is relative to the rows area.
            let rows_top = editor_ui::sidebar::HEADER_HEIGHT * self.scale_factor;
            if let Some(idx) = self.sidebar.row_index_at_y(y as f32, self.scale_factor, rows_top) {
                let row = self.sidebar.flat_rows()[idx].clone();
                self.sidebar.highlighted = Some(row.rel.clone());
                if row.is_dir {
                    self.sidebar.toggle_dir(&row.rel);
                    self.sidebar.rebuild_flat(&self.workspace_entries);
                } else if let Some(root) = self.workspace_root() {
                    let abs = root.join(&row.rel);
                    self.open_path_as_buffer(&abs);
                }
                self.request_redraw();
            }
            return;
        }

        // Tab strip clicks.
        let tab_h = self.tabstrip_height_px() as f64;
        if tab_h > 0.0 && y < tab_h {
            let xf = x as f32;
            if let Some(hit) =
                self.tab_hits.iter().find(|h| xf >= h.x0 && xf <= h.close_x1).cloned()
            {
                if xf >= hit.close_x0 && xf <= hit.close_x1 {
                    // Close button — refuse if dirty.
                    if self.buffers.get(hit.id).map(|s| s.dirty).unwrap_or(false)
                        && self.active_buffer_id == Some(hit.id)
                        && self.dirty
                    {
                        warn!(
                            "close tab: refusing to close dirty buffer without save (Ctrl+S first)"
                        );
                    } else if hit.id == self.active_buffer_id.unwrap_or(BufferId(u64::MAX)) {
                        // Closing the active buffer: run CloseBuffer path.
                        let _ = self.apply_editor_command(EditorCommand::CloseBuffer);
                    } else if self.buffers.close(hit.id, false).is_err() {
                        warn!("close tab: refusing to close dirty inactive buffer");
                    }
                    self.request_redraw();
                    return;
                }
                self.switch_to_buffer(hit.id);
                self.request_redraw();
            }
            return;
        }

        if self.terminal_pane_visible && self.pointer_in_terminal_pane(y) {
            self.terminal_focus = true;
            return;
        }
        self.terminal_focus = false;
        self.sidebar.focused = false;
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
        let Some(cb) = self.editor_content_bottom_px() else {
            return;
        };
        if y >= cb {
            return;
        }
        self.autoscroll_drag_edges(y);
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
        if let Err(e) = self.settings_store.flush_pending_save() {
            warn!(error = %e, "settings: debounced save failed");
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

    /// When the file on disk changed since load (`external_modified`), ask before overwriting (M25).
    fn confirm_save_if_externally_modified(&mut self) -> bool {
        if !self.external_modified {
            return true;
        }
        let mut dlg = MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title("File changed on disk")
            .set_description(
                "This file was modified outside the editor since it was opened.\n\n\
                 • Yes — save the editor buffer (overwrite the file on disk)\n\
                 • No — reload from disk (discard unsaved editor changes)\n\
                 • Cancel — do not save",
            )
            .set_buttons(MessageButtons::YesNoCancel);
        if let Some(w) = self.window.as_ref() {
            dlg = dlg.set_parent(w.as_ref());
        }
        match dlg.show() {
            MessageDialogResult::Yes => true,
            MessageDialogResult::No => {
                self.reload_from_disk_best_effort();
                false
            }
            MessageDialogResult::Cancel
            | MessageDialogResult::Ok
            | MessageDialogResult::Custom(_) => false,
        }
    }

    fn reload_from_disk_best_effort(&mut self) {
        let Some(ref path) = self.open_path else {
            return;
        };
        match load_file_sync(path) {
            Ok(l) => {
                info!(path = %path.display(), "reloaded buffer from disk after external change");
                self.apply_loaded(l);
            }
            Err(e) => warn!(path = %path.display(), error = %e, "reload from disk failed"),
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
        if self.external_modified && !self.confirm_save_if_externally_modified() {
            return;
        }
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
        self.persisted.sidebar_width = Some(self.sidebar.width);
        self.persisted.sidebar_visible = Some(self.sidebar.visible);
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
            status_message: None,
            git_branch: self.git_branch.clone(),
            git_modified_count: None,
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
        self.poll_workspace_events();
        self.refresh_git_branch(false);
        // Keep the BufferManager snapshot of the active buffer in sync so the tab strip
        // renders the correct label and dirty marker.
        self.sync_active_metadata_only();
        let status = self.frame_status();
        let dev_hud_line = self.dev_hud.then(|| self.metrics.hud_line());
        let term_snap_owned = if self.terminal_pane_visible {
            let idx = self.active_terminal_slot_idx();
            self.terminals[idx].as_ref().map(|t| t.emulator().lock().render_snapshot())
        } else {
            None
        };
        let terminal_pane_height_px = self.terminal_pane_height_px();

        // Build chrome (sidebar + tab strip + quick-open overlay).
        let (chrome_opt, inset_left_px, inset_top_px, tab_hits) = self.build_frame_chrome();
        self.tab_hits = tab_hits;

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
                diff: None,
                terminal_pane_height_px,
                terminal_snapshot: term_snap_owned,
                settings_overlay_lines: self.settings_overlay_lines.as_deref(),
                frame_chrome: chrome_opt.as_ref(),
                content_inset_left_px: inset_left_px,
                content_inset_top_px: inset_top_px,
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

    /// Build sidebar + tab strip + quick-open chrome for the current frame.
    /// Returns (Some(chrome), sidebar_width_px, tab_strip_height_px, tab_hits) when any
    /// surface is visible, else (None, 0, 0, []) so the renderer can skip chrome entirely.
    fn build_frame_chrome(&mut self) -> (Option<FrameChrome>, f32, f32, Vec<TabHit>) {
        // Diff-vs-HEAD cache is version-gated; cheap no-op when stable.
        self.refresh_gutter_marks_if_stale();
        let Some(window) = self.window.as_ref() else {
            return (None, 0.0, 0.0, Vec::new());
        };
        let physical = window.inner_size();
        let scale = self.scale_factor;

        let sidebar_on = self.sidebar.visible;
        let tabstrip_on = !self.buffers.is_empty();
        let breadcrumbs_on = tabstrip_on && self.active_path_rel().is_some();
        let find_on = self.find_bar.visible;

        let mut chrome = FrameChrome::new();
        let activity_w = ACTIVITY_BAR_WIDTH * scale;
        let sidebar_w = if sidebar_on { self.sidebar.width * scale } else { 0.0 };
        let inset_left_px = activity_w + sidebar_w;
        let tab_top_px = if tabstrip_on { TAB_STRIP_HEIGHT * scale } else { 0.0 };
        let breadcrumbs_top_px = tab_top_px;
        let breadcrumbs_height_px =
            if breadcrumbs_on { editor_ui::BREADCRUMBS_HEIGHT * scale } else { 0.0 };
        let inset_top_px = tab_top_px + breadcrumbs_height_px;

        let status_h = self.status_bar_height_px();
        let term_h = self.terminal_pane_height_px();
        let column_h = (physical.height as f32 - status_h - term_h).max(1.0);

        // Activity bar: always visible minimal shell.
        let icons = [
            ActivityIcon::new(editor_ui::Icon::Explorer, sidebar_on),
            ActivityIcon::new(editor_ui::Icon::Search, false),
            ActivityIcon::new(editor_ui::Icon::SourceControl, false),
            ActivityIcon::new(editor_ui::Icon::Run, false),
            ActivityIcon::new(editor_ui::Icon::Settings, false),
        ];
        paint_activity_bar(&mut chrome, scale, column_h, &icons);

        // Sidebar column: right of the activity bar.
        if sidebar_on {
            let auto =
                if let (Some(abs), Some(root)) = (self.open_path.as_ref(), self.workspace_root()) {
                    abs.strip_prefix(root).ok().map(Path::to_path_buf)
                } else {
                    None
                };
            self.sidebar.paint(
                &mut chrome,
                &self.buffers,
                self.workspace_root(),
                auto.as_deref(),
                scale,
                activity_w,
                0.0,
                column_h,
            );
        }

        // Tab strip: spans right of the sidebar.
        let mut tab_hits = Vec::new();
        if tabstrip_on {
            let strip_w = (physical.width as f32 - inset_left_px).max(0.0);
            tab_hits = paint_tab_strip(
                &mut chrome,
                &self.buffers,
                scale,
                inset_left_px,
                0.0,
                0.0,
                strip_w,
            );
        }

        // Breadcrumbs strip directly under the tab strip.
        if breadcrumbs_on {
            let strip_w = (physical.width as f32 - inset_left_px).max(0.0);
            let rel = self.active_path_rel();
            let _ = editor_ui::paint_breadcrumbs(
                &mut chrome,
                scale,
                inset_left_px,
                breadcrumbs_top_px,
                strip_w,
                rel.as_deref(),
            );
        }

        // Vertical scrollbar (always painted when the doc overflows the viewport).
        if let Some(renderer) = self.renderer.as_ref() {
            let line_h = renderer.line_height_px();
            let input = editor_ui::ScrollbarInput {
                total_lines: self.buffer.len_lines().max(1),
                scroll_y_px: self.scroll.y_px,
                line_height_px: line_h,
                content_right_px: physical.width as f32,
                content_top_px: inset_top_px,
                content_bottom_px: (physical.height as f32 - status_h - term_h).max(inset_top_px),
                scale,
            };
            let _ = editor_ui::paint_scrollbar(&mut chrome, input);
        }

        // Diff marks: colored stripes at the left edge of the gutter showing
        // +/~/- vs HEAD. Cache is pre-refreshed at render-time; see the
        // request_redraw() path for `refresh_gutter_marks_if_stale`.
        if let Some(renderer) = self.renderer.as_ref() {
            if !self.gutter_marks.is_empty() {
                let line_h = renderer.line_height_px();
                let total = self.buffer.len_lines().max(1);
                let first = (self.scroll.y_px / line_h).floor().max(0.0) as usize;
                let viewport_h =
                    (physical.height as f32 - status_h - term_h - inset_top_px).max(0.0);
                let visible = (viewport_h / line_h).ceil() as usize + 2;
                let last = (first + visible).min(total);
                let row_top_px = inset_top_px + 4.0 + (first as f32 * line_h - self.scroll.y_px);
                // Stripe sits just inside the left chrome edge (before the line numbers).
                let stripe_left = inset_left_px + 2.0 * scale;
                editor_ui::paint_gutter_marks(
                    &mut chrome,
                    &self.gutter_marks,
                    first..last,
                    stripe_left,
                    row_top_px,
                    line_h,
                    scale,
                );
            }
        }

        // Find bar: backdrop + highlight quads for visible matches + overlay text.
        if find_on {
            self.paint_find_bar_into_chrome(&mut chrome, inset_left_px, inset_top_px, physical);
        }

        // Quick-open palette: dim overlay + centered card.
        if self.quick_open.visible {
            self.quick_open.paint(
                &mut chrome,
                scale,
                physical.width as f32,
                physical.height as f32,
            );
        }

        // Command palette (Ctrl+Shift+P).
        if self.command_palette.visible {
            self.command_palette.paint(
                &mut chrome,
                scale,
                physical.width as f32,
                physical.height as f32,
            );
        }

        // Status bar accent — blue when a workspace is open (VS Code convention), else neutral.
        let status_rgba: [f32; 4] = if self.workspace.is_some() {
            [0.0, 0.48, 0.80, 1.0] // #007acc
        } else {
            [0.20, 0.20, 0.21, 1.0] // #333333
        };
        chrome.push_quad(ChromeQuad {
            left: 0.0,
            top: physical.height as f32 - status_h,
            width: physical.width as f32,
            height: status_h,
            rgba: status_rgba,
        });

        (Some(chrome), inset_left_px, inset_top_px, tab_hits)
    }

    /// Paint M16 find-bar backdrop, overlay text, and match highlights into `chrome`.
    fn paint_find_bar_into_chrome(
        &self,
        chrome: &mut FrameChrome,
        inset_left_px: f32,
        inset_top_px: f32,
        physical: PhysicalSize<u32>,
    ) {
        let scale = self.scale_factor;
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let line_h = renderer.line_height_px();
        let (gutter_w, char_w) =
            editor_render::compute_gutter_width_px(self.buffer.len_lines(), scale);
        let body_left = inset_left_px + 8.0 + gutter_w;
        let body_top = inset_top_px;
        let status_h = self.status_bar_height_px();
        let term_h = self.terminal_pane_height_px();
        let content_bottom = physical.height as f32 - status_h - term_h;

        // 1) Match highlights — semi-transparent tint per visible match.
        let scroll_y = self.scroll.y_px;
        let current = self.find_bar.current_match;
        for (i, m) in self.find_bar.matches.iter().enumerate() {
            let start = m.byte_range.start;
            let end = m.byte_range.end;
            let Ok(start_lc) = self.buffer.byte_to_line_col(BytePos(start)) else {
                continue;
            };
            let Ok(end_lc) = self.buffer.byte_to_line_col(BytePos(end)) else {
                continue;
            };
            let rgba =
                if Some(i) == current { [1.0, 0.75, 0.1, 0.55] } else { [1.0, 0.9, 0.3, 0.32] };
            // One rect per line the match spans.
            for line in start_lc.line..=end_lc.line {
                let line_top = body_top + 4.0 + (line as f32) * line_h - scroll_y;
                let line_bot = line_top + line_h;
                if line_bot < body_top || line_top > content_bottom {
                    continue;
                }
                let col_start = if line == start_lc.line { start_lc.col } else { 0 };
                let col_end = if line == end_lc.line {
                    end_lc.col
                } else {
                    self.buffer.line_len_bytes(line).unwrap_or(col_start)
                };
                let left = body_left + (col_start as f32) * char_w;
                let width = ((col_end - col_start) as f32).max(0.0) * char_w;
                if width <= 0.0 {
                    continue;
                }
                chrome.push_quad(ChromeQuad {
                    left,
                    top: line_top.max(body_top),
                    width,
                    height: (line_bot.min(content_bottom) - line_top.max(body_top)).max(0.0),
                    rgba,
                });
            }
        }

        // 2) Find bar backdrop — dark strip just below the tab strip, spanning the editor body.
        let backdrop_top = body_top;
        let backdrop_height = self.find_bar.backdrop_height_px(scale);
        chrome.push_quad(ChromeQuad {
            left: inset_left_px,
            top: backdrop_top,
            width: (physical.width as f32 - inset_left_px).max(0.0),
            height: backdrop_height,
            rgba: [0.15, 0.15, 0.17, 0.96],
        });

        // 3) Overlay text (title, flags + Find field, Replace field, match count).
        let overlay = self.find_bar.format_overlay(self.blink_on);
        for (row, line_text) in overlay.lines().enumerate() {
            let row_top = backdrop_top + 4.0 * scale + (row as f32) * (12.0 * scale);
            if row_top + (12.0 * scale) > backdrop_top + backdrop_height {
                break;
            }
            chrome.push_line(
                inset_left_px + 8.0 * scale,
                row_top,
                line_text.to_string(),
                [0xdc, 0xdc, 0xdc],
            );
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
                if self.settings_overlay_lines.is_some() {
                    self.settings_overlay_lines = None;
                    self.request_redraw();
                    return false;
                }
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
            EditorCommand::ToggleFullscreen => {
                if let Some(w) = self.window.as_ref() {
                    if w.fullscreen().is_some() {
                        w.set_fullscreen(None);
                    } else {
                        w.set_fullscreen(Some(Fullscreen::Borderless(None)));
                    }
                }
                false
            }
            EditorCommand::ToggleTerminalPane => {
                self.terminal_pane_visible = !self.terminal_pane_visible;
                self.terminal_split_dragging = false;
                if !self.terminal_pane_visible {
                    self.terminal_focus = false;
                } else {
                    self.ensure_terminal_spawned();
                    self.sync_terminal_size();
                }
                self.request_redraw();
                false
            }
            EditorCommand::NewIntegratedTerminal => {
                self.terminal_pane_visible = true;
                self.ensure_terminal_spawned();
                if self.terminals[1].is_none() {
                    if let Err(e) = self.spawn_terminal(1) {
                        warn!(error = %e, "spawn second integrated terminal");
                    }
                }
                self.active_terminal_slot = 1;
                self.sync_terminal_size();
                self.request_redraw();
                false
            }
            EditorCommand::OpenSettings => {
                self.settings_overlay_lines = Some(format_settings_overlay(&self.settings_store));
                self.request_redraw();
                false
            }
            EditorCommand::Cancel => {
                if self.find_bar.visible {
                    self.find_bar.visible = false;
                    self.find_bar.matches.clear();
                    self.find_bar.current_match = None;
                    self.find_bar.regex_error = None;
                    self.request_redraw();
                    return false;
                }
                if self.quick_open.visible {
                    self.quick_open.hide();
                    self.request_redraw();
                    return false;
                }
                if self.command_palette.visible {
                    self.command_palette.hide();
                    self.request_redraw();
                    return false;
                }
                if self.sidebar.focused {
                    self.sidebar.focused = false;
                    self.request_redraw();
                    return false;
                }
                if self.settings_overlay_lines.take().is_some() {
                    self.request_redraw();
                }
                false
            }
            EditorCommand::ToggleSidebar => {
                self.sidebar.visible = !self.sidebar.visible;
                if self.sidebar.visible && self.workspace.is_none() {
                    // First-time open with no workspace — prompt for one so the tree isn't empty.
                    if let Some(root) = rfd::FileDialog::new().pick_folder() {
                        self.open_workspace_folder(&root);
                    } else {
                        // User cancelled — roll the toggle back so we don't leave an empty pane.
                        self.sidebar.visible = false;
                    }
                }
                self.sidebar.focused = self.sidebar.visible;
                self.request_redraw();
                false
            }
            EditorCommand::FocusSidebar => {
                self.sidebar.visible = true;
                self.sidebar.focused = true;
                if self.sidebar.highlighted.is_none() {
                    if let Some(row) = self.sidebar.flat_rows().first() {
                        self.sidebar.highlighted = Some(row.rel.clone());
                    }
                }
                self.request_redraw();
                false
            }
            EditorCommand::ToggleQuickOpen => {
                if self.workspace.is_none() {
                    if let Some(root) = rfd::FileDialog::new().pick_folder() {
                        self.open_workspace_folder(&root);
                    }
                }
                if self.workspace.is_some() {
                    if self.quick_open.visible {
                        self.quick_open.hide();
                    } else {
                        self.quick_open.show();
                    }
                }
                self.request_redraw();
                false
            }
            EditorCommand::OpenCommandPalette => {
                self.ensure_command_palette_seeded();
                if self.command_palette.visible {
                    self.command_palette.hide();
                } else {
                    self.command_palette.clear_query();
                    self.command_palette.show();
                }
                self.request_redraw();
                false
            }
            EditorCommand::NextBuffer => {
                if self.buffers.len() > 1 {
                    if let Some(cur) = self.active_buffer_id {
                        self.sync_active_to_manager_with_id(cur);
                    }
                    self.buffers.next_buffer();
                    if let Some(id) = self.buffers.active() {
                        self.load_state_from_buffer(id);
                        self.reveal_active_in_sidebar();
                    }
                }
                false
            }
            EditorCommand::PrevBuffer => {
                if self.buffers.len() > 1 {
                    if let Some(cur) = self.active_buffer_id {
                        self.sync_active_to_manager_with_id(cur);
                    }
                    self.buffers.prev_buffer();
                    if let Some(id) = self.buffers.active() {
                        self.load_state_from_buffer(id);
                        self.reveal_active_in_sidebar();
                    }
                }
                false
            }
            EditorCommand::CloseBuffer => {
                let Some(id) = self.active_buffer_id else {
                    return false;
                };
                if self.dirty {
                    warn!("close buffer: refusing to close dirty buffer without save (press Ctrl+S first)");
                    return false;
                }
                if self.buffers.close(id, false).is_ok() {
                    self.active_buffer_id = None;
                    if let Some(next) = self.buffers.active() {
                        self.load_state_from_buffer(next);
                        self.reveal_active_in_sidebar();
                    } else {
                        // No buffers left — reset to an untitled empty one so the window
                        // still has something to render.
                        let new_id =
                            self.buffers.create_empty_coalesced(self.persisted.undo_coalesce_ms);
                        self.buffer = TextBuffer::new();
                        self.cursor = Cursor::new(BytePos(0));
                        self.selection = Selection::empty(BytePos(0));
                        self.undo = UndoStack::default();
                        self.scroll = ScrollOffset::default();
                        self.open_path = None;
                        self.disk_encoding = Encoding::Utf8;
                        self.dirty = false;
                        self.external_modified = false;
                        self.file_mtime = None;
                        self.active_buffer_id = Some(new_id);
                    }
                    self.sync_window_title();
                }
                false
            }
            EditorCommand::NewBuffer => {
                if let Some(cur) = self.active_buffer_id {
                    self.sync_active_to_manager_with_id(cur);
                }
                let id = self.buffers.create_empty_coalesced(self.persisted.undo_coalesce_ms);
                self.buffer = TextBuffer::new();
                self.cursor = Cursor::new(BytePos(0));
                self.selection = Selection::empty(BytePos(0));
                self.undo = UndoStack::default();
                self.scroll = ScrollOffset::default();
                self.open_path = None;
                self.disk_encoding = Encoding::Utf8;
                self.dirty = false;
                self.external_modified = false;
                self.file_mtime = None;
                self.active_buffer_id = Some(id);
                self.sync_window_title();
                false
            }
            EditorCommand::FindInFile => {
                self.open_find_bar(false);
                false
            }
            EditorCommand::ReplaceInFile => {
                self.open_find_bar(true);
                false
            }
            EditorCommand::FindNext => {
                if !self.find_bar.visible {
                    self.open_find_bar(false);
                }
                if !self.find_bar.query.is_empty() {
                    self.find_bar.rerun_search(&self.buffer.snapshot());
                    self.find_bar.next_match();
                    self.reveal_current_match();
                }
                self.request_redraw();
                false
            }
            EditorCommand::FindPrev => {
                if !self.find_bar.visible {
                    self.open_find_bar(false);
                }
                if !self.find_bar.query.is_empty() {
                    self.find_bar.rerun_search(&self.buffer.snapshot());
                    self.find_bar.prev_match();
                    self.reveal_current_match();
                }
                self.request_redraw();
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
            AppEvent::TerminalPump => {
                if self.terminal_pane_visible {
                    let mut dirty = false;
                    for t in self.terminals.iter_mut().flatten() {
                        if t.poll() {
                            dirty = true;
                        }
                    }
                    if dirty {
                        self.request_redraw();
                    }
                }
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
                self.sync_terminal_size();
                self.paint_frame();
            }
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(r)) = (&self.window, self.renderer.as_mut()) {
                    r.resize(size);
                    r.sync_present_mode(w);
                }
                self.clamp_scroll();
                self.sync_terminal_size();
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
                if self.terminal_split_dragging {
                    self.update_terminal_split_from_pointer_y(position.y);
                    self.sync_terminal_size();
                    self.request_redraw();
                    return;
                }
                if let Some(cmd) = self.mouse_chord.on_cursor_moved(position) {
                    let quit = self.apply_editor_command(cmd);
                    self.sync_window_title();
                    self.request_redraw();
                    if quit {
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } if button == MouseButton::Left => {
                match state {
                    ElementState::Pressed
                        if self.terminal_pane_visible
                            && self.pointer_on_terminal_divider(self.last_pointer.y) =>
                    {
                        self.terminal_split_dragging = true;
                        self.update_terminal_split_from_pointer_y(self.last_pointer.y);
                        self.sync_terminal_size();
                        self.request_redraw();
                        return;
                    }
                    ElementState::Released => {
                        self.terminal_split_dragging = false;
                    }
                    ElementState::Pressed => {}
                }
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
            WindowEvent::MouseInput { .. } => {}
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = scroll_delta_y_pixels(delta, self.scale_factor);
                if dy == 0.0 {
                    return;
                }
                if self.terminal_focus
                    && self.terminal_pane_visible
                    && self.pointer_in_terminal_pane(self.last_pointer.y)
                {
                    let line_h = self.renderer.as_ref().map(|r| r.line_height_px()).unwrap_or(20.0);
                    let lines = (-dy / line_h).round() as i32;
                    if lines != 0 {
                        let slot = self.active_terminal_slot_idx();
                        if let Some(ref t) = self.terminals[slot] {
                            t.emulator().lock().scroll_lines(lines);
                        }
                    }
                    self.request_redraw();
                    return;
                }
                let quit =
                    self.apply_editor_command(EditorCommand::ScrollContent { delta_y_px: dy });
                self.sync_window_title();
                self.request_redraw();
                if quit {
                    event_loop.exit();
                }
            }
            WindowEvent::Ime(ime) => match ime {
                Ime::Enabled | Ime::Disabled => {}
                Ime::Preedit(_, _) => {
                    self.request_redraw();
                }
                Ime::Commit(text) => {
                    if self.find_bar.visible && !text.is_empty() {
                        let (field, cursor) = if self.find_bar.focus_replace {
                            (&mut self.find_bar.replace, &mut self.find_bar.replace_cursor)
                        } else {
                            (&mut self.find_bar.query, &mut self.find_bar.query_cursor)
                        };
                        field.insert_str(*cursor, text.as_str());
                        *cursor += text.len();
                        if !self.find_bar.focus_replace {
                            self.find_bar.rerun_search(&self.buffer.snapshot());
                        }
                        self.request_redraw();
                        return;
                    }
                    if self.quick_open.visible && !text.is_empty() {
                        for ch in text.chars() {
                            self.quick_open.push_char(ch);
                        }
                        self.request_redraw();
                        return;
                    }
                    if self.command_palette.visible && !text.is_empty() {
                        for ch in text.chars() {
                            self.command_palette.push_char(ch);
                        }
                        self.request_redraw();
                        return;
                    }
                    if self.terminal_focus
                        && self.terminal_pane_visible
                        && self.settings_overlay_lines.is_none()
                    {
                        if !text.is_empty() {
                            let slot = self.active_terminal_slot_idx();
                            if let Some(ref mut t) = self.terminals[slot] {
                                if let Err(e) = t.write_bytes(text.as_bytes()) {
                                    warn!(slot, error = %e, "terminal IME write failed");
                                }
                            }
                        }
                        self.request_redraw();
                        return;
                    }
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
                if self.find_bar.visible && self.handle_find_bar_key(&event) {
                    self.sync_window_title();
                    self.request_redraw();
                    return;
                }
                if self.quick_open.visible && self.handle_quick_open_key(&event) {
                    self.request_redraw();
                    return;
                }
                if self.command_palette.visible && self.handle_command_palette_key(&event) {
                    self.request_redraw();
                    return;
                }
                if self.handle_sidebar_key(&event) {
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
