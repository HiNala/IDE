//! Errors from workspace setup and walks.

use thiserror::Error;

/// Opening a workspace or scanning the tree failed.
#[derive(Debug, Error)]
pub enum WorkspaceError {
    /// I/O (canonicalize, metadata, walk).
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Building `.gitignore` / exclude rules.
    #[error(transparent)]
    Ignore(#[from] ignore::Error),
    /// File watcher (`notify` crate).
    #[error(transparent)]
    Notify(#[from] notify::Error),
}
