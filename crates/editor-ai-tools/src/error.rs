//! Errors from tools and transactions.

use std::path::PathBuf;

use thiserror::Error;

/// Tool or workspace transaction failure.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("path escapes workspace: {0}")]
    PathEscape(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("editor core: {0}")]
    Core(#[from] editor_core::CoreError),
    #[error("load error: {0}")]
    Load(#[from] editor_io::LoadError),
    #[error("save error: {0}")]
    Save(#[from] editor_io::SaveError),
    #[error("workspace: {0}")]
    Workspace(#[from] editor_workspace::WorkspaceError),
    #[error("search: {0}")]
    Search(#[from] editor_search::SearchError),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
    #[error("shell disabled or command not allowed: {0}")]
    ShellDenied(String),
    #[error("file too large (max {max} bytes): {path:?}")]
    FileTooLarge { path: PathBuf, max: usize },
    #[error("buffer not found for path (internal)")]
    BufferMissing,
}

impl ToolError {
    pub fn msg(s: impl Into<String>) -> Self {
        ToolError::Message(s.into())
    }
}

pub type Result<T, E = ToolError> = std::result::Result<T, E>;
