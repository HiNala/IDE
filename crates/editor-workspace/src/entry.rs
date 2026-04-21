//! File entries returned by workspace walks.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// High-level filesystem kind for sidebar / tree (M13+).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Regular,
    Directory,
    Symlink,
}

/// One regular file under a workspace root.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub relative: PathBuf,
    pub kind: FileKind,
    pub size_bytes: u64,
    pub mtime: SystemTime,
    pub is_binary_heuristic: bool,
}

const SNIFF_BYTES: usize = 8 * 1024;

/// Cheap newline / UTF-8 heuristic so the UI can warn before opening as text.
#[must_use]
pub fn is_binary_heuristic(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() == 0 {
        return false;
    }
    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let len = meta.len() as usize;
    let take = len.min(SNIFF_BYTES);
    let mut buf = vec![0u8; take];
    let n = f.read(&mut buf).unwrap_or(0);
    buf.truncate(n);
    if buf.contains(&0) {
        return true;
    }
    std::str::from_utf8(&buf).is_err()
}
