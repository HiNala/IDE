//! Combined PTY + emulator + output pump.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use parking_lot::Mutex;
use portable_pty::{ExitStatus, PtySize};

use crate::emulator::{ProxyListener, TerminalEmulator};
use crate::error::TerminalError;
use crate::events::TerminalEvent;
use crate::pty::{PtyReadMsg, TerminalProcess};
use crate::shell::ShellConfig;

/// Stable handle for UI / agent integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalId(pub u64);

/// Configuration for [`Terminal::spawn`].
#[derive(Debug)]
pub struct TerminalConfig {
    pub id: TerminalId,
    pub shell: ShellConfig,
    pub cwd: PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub cell_width_px: u16,
    pub cell_height_px: u16,
}

/// Result of [`Terminal::run_command`].
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub output: String,
    pub timed_out: bool,
}

/// Integrated PTY + alacritty terminal.
pub struct Terminal {
    id: TerminalId,
    process: TerminalProcess,
    emulator: Arc<Mutex<TerminalEmulator>>,
    out_rx: Receiver<PtyReadMsg>,
    event_rx: Receiver<TerminalEvent>,
    dirty: Arc<AtomicBool>,
    title: String,
    exited: Option<ExitStatus>,
    capture: Arc<Mutex<Vec<u8>>>,
    capture_max: usize,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal").field("id", &self.id).finish_non_exhaustive()
    }
}

impl Terminal {
    /// Spawn a shell in a PTY and wire the emulator.
    pub fn spawn(config: TerminalConfig) -> Result<Self, TerminalError> {
        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        let listener = ProxyListener(event_tx);
        let emulator =
            TerminalEmulator::new(config.cols as usize, config.rows as usize, 10_000, listener);
        let emu = Arc::new(Mutex::new(emulator));
        let dirty = Arc::new(AtomicBool::new(true));
        let capture = Arc::new(Mutex::new(Vec::new()));

        let size = PtySize {
            rows: config.rows,
            cols: config.cols,
            pixel_width: config.cell_width_px.saturating_mul(config.cols),
            pixel_height: config.cell_height_px.saturating_mul(config.rows),
        };

        let (process, out_rx) = TerminalProcess::spawn(&config.shell, config.cwd, size)?;

        Ok(Self {
            id: config.id,
            process,
            emulator: emu,
            out_rx,
            event_rx,
            dirty,
            title: String::new(),
            exited: None,
            capture,
            capture_max: 4 * 1024 * 1024,
        })
    }

    #[must_use]
    pub fn id(&self) -> TerminalId {
        self.id
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        self.process.write(bytes)
    }

    pub fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        cell_width_px: u16,
        cell_height_px: u16,
    ) -> Result<(), TerminalError> {
        let size = PtySize {
            rows,
            cols,
            pixel_width: cell_width_px.saturating_mul(cols),
            pixel_height: cell_height_px.saturating_mul(rows),
        };
        self.process.resize(size)?;
        self.emulator.lock().resize(cols as usize, rows as usize);
        self.dirty.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Drain PTY + event channels; returns `true` if something changed visually.
    pub fn poll(&mut self) -> bool {
        let mut redraw = false;

        while let Ok(msg) = self.out_rx.try_recv() {
            match msg {
                PtyReadMsg::Data(bytes) => {
                    self.push_capture_bytes(&bytes);
                    self.emulator.lock().process_bytes(&bytes);
                    redraw = true;
                }
                PtyReadMsg::Eof => tracing::debug!("PTY reader EOF"),
            }
        }

        while let Ok(ev) = self.event_rx.try_recv() {
            match ev {
                TerminalEvent::Dirty => redraw = true,
                TerminalEvent::TitleChanged(t) => {
                    self.title = t;
                    redraw = true;
                }
                TerminalEvent::Bell => redraw = true,
                TerminalEvent::Exited(_) => redraw = true,
            }
        }

        if let Ok(Some(st)) = self.process.try_wait() {
            if self.exited.is_none() {
                redraw = true;
            }
            self.exited = Some(st);
        }

        if redraw {
            self.dirty.store(true, Ordering::SeqCst);
        }

        redraw
    }

    fn push_capture_bytes(&self, bytes: &[u8]) {
        let mut cap = self.capture.lock();
        if cap.len() + bytes.len() > self.capture_max {
            let drop = cap.len() / 4;
            cap.drain(..drop);
        }
        cap.extend_from_slice(bytes);
    }

    /// Returns whether the grid changed since last call (clears the internal flag).
    #[must_use]
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::SeqCst)
    }

    #[must_use]
    pub fn emulator(&self) -> Arc<Mutex<TerminalEmulator>> {
        Arc::clone(&self.emulator)
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn exited(&self) -> Option<ExitStatus> {
        self.exited.clone()
    }

    pub fn kill(&mut self) {
        self.process.kill();
    }

    /// Run one shell line; waits for a rough prompt heuristic or `timeout` (M20).
    pub fn run_command(
        &mut self,
        command: &str,
        timeout: Duration,
    ) -> mpsc::Receiver<Result<CommandResult, TerminalError>> {
        let (tx, rx) = mpsc::channel();
        let line = format!("{}\n", command.trim_end());
        let start_len = self.capture.lock().len();
        if let Err(e) = self.write_bytes(line.as_bytes()) {
            let _ = tx.send(Err(e));
            return rx;
        }

        let cap = Arc::clone(&self.capture);
        std::thread::spawn(move || {
            let start = Instant::now();
            while start.elapsed() < timeout {
                std::thread::sleep(Duration::from_millis(50));
                let buf = cap.lock();
                let slice = buf.get(start_len..).unwrap_or(&[]);
                if slice.is_empty() {
                    continue;
                }
                if let Ok(text) = std::str::from_utf8(slice) {
                    if prompt_heuristic(text) {
                        let _ = tx
                            .send(Ok(CommandResult { output: text.to_string(), timed_out: false }));
                        return;
                    }
                }
            }
            let tail = cap.lock();
            let slice = tail.get(start_len..).unwrap_or(&[]);
            let output = String::from_utf8_lossy(slice).into_owned();
            let _ = tx.send(Ok(CommandResult { output, timed_out: true }));
        });

        rx
    }
}

fn prompt_heuristic(s: &str) -> bool {
    let t = s.trim_end();
    if t.is_empty() {
        return false;
    }
    if let Some(line) = t.lines().last() {
        let l = line.trim_end();
        l.ends_with('$')
            || l.ends_with('>')
            || l.ends_with('#')
            || l.contains("PS ")
            || l.starts_with("C:\\")
    } else {
        false
    }
}
