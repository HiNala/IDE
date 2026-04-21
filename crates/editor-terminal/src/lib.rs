//! Integrated terminal: `portable-pty` + `alacritty_terminal` (M26).
//!
//! PTY reads run on a background thread; the UI thread drains bytes into the emulator.

#![forbid(unsafe_code)]

mod color_resolve;
mod emulator;
mod error;
mod events;
mod input;
mod pty;
mod shell;
mod terminal;

pub use emulator::{TerminalEmulator, TerminalRenderSnapshot, TerminalRowRuns};
pub use error::TerminalError;
pub use events::TerminalEvent;
pub use input::encode_key;
pub use portable_pty::{ExitStatus, PtySize};
pub use pty::TerminalProcess;
pub use shell::{detect_shell, ShellConfig};
pub use terminal::{CommandResult, Terminal, TerminalConfig, TerminalId};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[must_use]
pub fn banner() -> String {
    format!("editor-terminal v{VERSION}")
}
