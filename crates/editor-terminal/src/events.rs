//! Events emitted by the integrated terminal (title, bell, exit).

use portable_pty::ExitStatus;

/// User-visible terminal lifecycle / notification events.
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    /// Terminal grid changed; request redraw.
    Dirty,
    /// Shell / OSC set a new title string.
    TitleChanged(String),
    /// Child process exited.
    Exited(ExitStatus),
    /// Bell (visual flash only in V3).
    Bell,
}
