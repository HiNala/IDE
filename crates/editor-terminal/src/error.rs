//! Terminal subsystem errors.

use thiserror::Error;

/// Errors from PTY, I/O, or shell setup.
#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("PTY: {0}")]
    Pty(String),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("shell path is not valid UTF-8")]
    ShellPathEncoding,
}

impl From<anyhow::Error> for TerminalError {
    fn from(value: anyhow::Error) -> Self {
        TerminalError::Pty(value.to_string())
    }
}
