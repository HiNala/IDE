//! Workspace model: project root, gitignore-aware file discovery, `notify` watcher,
//! and multi-buffer bookkeeping ([`BufferManager`]) — headless foundation for M13+.

#![forbid(unsafe_code)]

pub mod buffers;
pub mod entry;
pub mod error;
pub mod workspace;

pub use buffers::{BufferId, BufferManager, BufferState, CloseError};
pub use entry::{is_binary_heuristic, FileEntry, FileKind};
pub use error::WorkspaceError;
pub use workspace::{
    path_has_tooling_noise, path_is_meta_sidecar, relative_is_under_dot_ide, FileSystemEvent,
    WalkOptions, Workspace,
};

/// Crate version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Human-readable banner for startup logs.
#[must_use]
pub fn banner() -> String {
    format!("editor-workspace v{VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_ok() {
        assert!(banner().contains("editor-workspace"));
    }
}
