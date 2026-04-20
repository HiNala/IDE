//! Public I/O types and errors.

use std::path::PathBuf;
use std::time::SystemTime;

use editor_core::TextBuffer;

/// On-disk text encoding we preserve across load/save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    LossyUtf8,
}

/// Successful file load result.
#[derive(Debug)]
pub struct LoadedFile {
    pub buffer: TextBuffer,
    pub path: PathBuf,
    pub encoding: Encoding,
    pub byte_size: u64,
    pub mtime: SystemTime,
    pub was_memory_mapped: bool,
}

/// Load failure.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("path is not a regular file")]
    NotAFile,
    #[error("path has no parent directory (cannot atomic-save here)")]
    NoParentDir,
    #[error("load cancelled")]
    Cancelled,
}

/// Save failure.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("path has no parent directory")]
    NoParentDir,
    #[error("Windows reserved device name: {0}")]
    ReservedName(String),
    #[error("persist temporary file: {0}")]
    Persist(std::io::Error),
}
