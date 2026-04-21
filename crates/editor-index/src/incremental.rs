//! Hooks for incremental updates (debounce timing + path classification).
//!
//! The UI should poll [`editor_workspace::Workspace::poll_events`], merge events for the same
//! path, wait [`INDEX_DEBOUNCE`] after the last event, then call [`Indexer::reindex_path`] on a
//! [`editor_core::WorkerPool`] — never on the render thread.

use std::path::Path;
use std::time::Duration;

use editor_workspace::{path_has_tooling_noise, path_is_meta_sidecar, FileSystemEvent};

/// Coalesce rapid save sequences (matches M22 mission default).
pub const INDEX_DEBOUNCE: Duration = Duration::from_millis(500);

#[must_use]
pub fn paths_from_fs_events(events: &[FileSystemEvent]) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in events {
        match e {
            FileSystemEvent::Created(p) | FileSystemEvent::Modified(p) => {
                // `.ide` is normally tooling noise for the explorer, but `.ide/meta/*.md` sidecars must re-index.
                if path_is_meta_sidecar(p) || !path_has_tooling_noise(p) {
                    out.push(p.clone());
                }
            }
            FileSystemEvent::Removed(p) => out.push(p.clone()),
            FileSystemEvent::Renamed { to, .. } => out.push(to.clone()),
        }
    }
    out.sort();
    out.dedup();
    out
}

#[must_use]
pub fn is_sidecar_path(p: &Path) -> bool {
    path_is_meta_sidecar(p)
}
