//! Project root, gitignore-aware enumeration, and debounced filesystem events.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossbeam_channel::Receiver;
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use notify::event::{Event, EventKind, ModifyKind};
use notify::RecursiveMode;
use notify_debouncer_full::notify::RecommendedWatcher;
use notify_debouncer_full::{
    new_debouncer, DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache,
};

use crate::entry::{is_binary_heuristic, FileEntry, FileKind};
use crate::WorkspaceError;

/// High-level change notifications (debounced, filtered).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSystemEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Removed(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

/// `true` for paths under VCS/tooling directories we do not want to spam the host with.
#[must_use]
pub fn path_has_tooling_noise(path: &Path) -> bool {
    path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        matches!(
            s.as_ref(),
            ".git" | ".ide" | "target" | "node_modules" | ".svn" | ".hg" | "dist" | "build"
        )
    })
}

/// `true` for `.ide/meta/**/*.md` sidecar files (M21). These are excluded from normal walks but
/// must still surface as filesystem events for the vector index (M22).
#[must_use]
pub fn path_is_meta_sidecar(path: &Path) -> bool {
    use std::path::Component;
    let mut has_ide = false;
    let mut has_meta = false;
    for c in path.components() {
        if let Component::Normal(part) = c {
            if part == ".ide" {
                has_ide = true;
            }
            if part == "meta" {
                has_meta = true;
            }
        }
    }
    has_ide && has_meta && path.extension().is_some_and(|e| e == "md")
}

fn configure_walk(root: &Path, include_ide_internals: bool) -> WalkBuilder {
    let mut b = WalkBuilder::new(root);
    b.standard_filters(true);
    // Apply `.gitignore` / `.ignore` even when the tree is not a git checkout (common for unsaved IDE projects).
    b.require_git(false);
    let mut overrides = OverrideBuilder::new(root);
    if include_ide_internals {
        // Allow `.ide` back in even though `standard_filters` hides dotfiles — we want the walker
        // to traverse project metadata without turning hidden filtering off globally.
        if overrides.add(".ide").is_ok() && overrides.add(".ide/**").is_ok() {
            if let Ok(ov) = overrides.build() {
                b.overrides(ov);
            }
        }
    } else {
        // Explicitly exclude `.ide/**` so the gitignore walker never yields these paths.
        if overrides.add("!.ide/**").is_ok() {
            if let Ok(ov) = overrides.build() {
                b.overrides(ov);
            }
        }
    }
    b
}

/// VCS / build output dirs omitted from explorer-style listings (`.ide` handled separately via [`WalkOptions`]).
#[must_use]
fn relative_is_tooling_dir_excluded(relative: &Path) -> bool {
    relative.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        matches!(s.as_ref(), ".git" | "target" | "node_modules" | ".svn" | ".hg" | "dist" | "build")
    })
}

/// Options for [`Workspace::walk_files_with_options`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WalkOptions {
    /// When `false` (default), paths under `.ide/` are omitted (sidebar / explorer).
    /// Search and grep are unchanged—they do not use this walk.
    pub include_ide_internals: bool,
}

#[must_use]
pub fn relative_is_under_dot_ide(relative: &Path) -> bool {
    matches!(
        relative.components().next(),
        Some(std::path::Component::Normal(name)) if name == std::ffi::OsStr::new(".ide")
    )
}

/// Open project at `root`: gitignore-aware walks + recursive file watcher (debounced).
pub struct Workspace {
    root: PathBuf,
    #[allow(dead_code)] // keeps OS watcher registered for the process lifetime
    debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    events_rx: Receiver<FileSystemEvent>,
}

impl fmt::Debug for Workspace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Workspace").field("root", &self.root).finish_non_exhaustive()
    }
}

impl Workspace {
    /// Open a workspace at `root` (directory). Watches recursively; enumeration is on demand via
    /// [`Self::walk_files`].
    pub fn open(root: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        let root = root.as_ref().canonicalize()?;
        if !root.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "workspace root must be a directory",
            )
            .into());
        }

        let (tx, events_rx) = crossbeam_channel::unbounded::<FileSystemEvent>();
        let root_for_cb = root.clone();

        let mut debouncer =
            new_debouncer(Duration::from_millis(200), None, move |res: DebounceEventResult| {
                match res {
                    Ok(events) => {
                        for ev in events {
                            for mapped in map_debounced(&root_for_cb, &ev) {
                                if tx.send(mapped).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(errs) => {
                        for e in errs {
                            tracing::warn!(error = ?e, "notify debouncer error batch");
                        }
                    }
                }
            })?;

        debouncer.watch(&root, RecursiveMode::Recursive).map_err(WorkspaceError::Notify)?;

        Ok(Self { root, debouncer, events_rx })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Drain pending filesystem events (call from the host frame loop).
    #[must_use]
    pub fn poll_events(&self) -> Vec<FileSystemEvent> {
        self.events_rx.try_iter().collect()
    }

    /// Gitignore-aware: uses the same rules as [`WalkBuilder`] / ripgrep.
    #[must_use]
    pub fn is_ignored(&self, path: &Path) -> bool {
        is_path_ignored(&self.root, path)
    }

    /// Full workspace scan (respects `.gitignore`, `.ignore`, `.git/info/exclude`, parent stacks).
    pub fn walk_files(&self) -> Result<Vec<FileEntry>, WorkspaceError> {
        self.walk_files_with_options(WalkOptions::default())
    }

    /// Like [`Self::walk_files`], with control over whether `.ide/**` is listed (for UI trees).
    pub fn walk_files_with_options(
        &self,
        opts: WalkOptions,
    ) -> Result<Vec<FileEntry>, WorkspaceError> {
        let mut out = Vec::new();

        for result in configure_walk(&self.root, opts.include_ide_internals).build() {
            let entry = result.map_err(WorkspaceError::Ignore)?;

            let path = entry.path().to_path_buf();
            let relative = path
                .strip_prefix(&self.root)
                .map(Path::to_path_buf)
                .unwrap_or_else(|_| PathBuf::from("."));

            if !opts.include_ide_internals && relative_is_under_dot_ide(&relative) {
                continue;
            }
            if relative_is_tooling_dir_excluded(&relative) {
                continue;
            }

            let meta = entry.metadata().map_err(WorkspaceError::Ignore)?;
            let ft = entry.file_type().unwrap_or_else(|| meta.file_type());

            let kind = if ft.is_symlink() {
                FileKind::Symlink
            } else if ft.is_dir() {
                FileKind::Directory
            } else {
                FileKind::Regular
            };

            let (size_bytes, mtime, is_binary) = if ft.is_file() {
                let sz = meta.len();
                let mt = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let bin = is_binary_heuristic(&path);
                (sz, mt, bin)
            } else if ft.is_dir() {
                (0, meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH), false)
            } else {
                let mt = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                (meta.len(), mt, false)
            };

            let n = out.len() + 1;
            if n > 10_000 && n % 1000 == 0 {
                tracing::info!(discovered = n, root = %self.root.display(), "workspace tree scan progress");
            }

            out.push(FileEntry {
                path,
                relative,
                kind,
                size_bytes,
                mtime,
                is_binary_heuristic: is_binary,
            });
        }

        Ok(out)
    }

    /// Borrowed iterator adapter for call sites that want `impl Iterator` (allocates via [`walk_files`]).
    pub fn iter_entries(&self) -> Result<impl Iterator<Item = FileEntry>, WorkspaceError> {
        self.walk_files().map(|v| v.into_iter())
    }

    /// Like [`Self::iter_entries`] with [`WalkOptions`].
    pub fn iter_entries_with_options(
        &self,
        opts: WalkOptions,
    ) -> Result<impl Iterator<Item = FileEntry>, WorkspaceError> {
        self.walk_files_with_options(opts).map(|v| v.into_iter())
    }
}

fn map_debounced(root: &Path, ev: &DebouncedEvent) -> Vec<FileSystemEvent> {
    let mut event = ev.event.clone();
    event
        .paths
        .retain(|p| p.starts_with(root) && (path_is_meta_sidecar(p) || !path_has_tooling_noise(p)));
    if event.paths.is_empty() {
        return Vec::new();
    }
    map_notify_event(&event)
}

fn map_notify_event(ev: &Event) -> Vec<FileSystemEvent> {
    let kind = &ev.kind;
    match kind {
        EventKind::Create(_) => ev.paths.iter().cloned().map(FileSystemEvent::Created).collect(),
        EventKind::Remove(_) => ev.paths.iter().cloned().map(FileSystemEvent::Removed).collect(),
        EventKind::Modify(ModifyKind::Name(_)) => {
            if ev.paths.len() >= 2 {
                return vec![FileSystemEvent::Renamed {
                    from: ev.paths[0].clone(),
                    to: ev.paths[1].clone(),
                }];
            }
            ev.paths.iter().cloned().map(FileSystemEvent::Modified).collect()
        }
        EventKind::Modify(_) => ev.paths.iter().cloned().map(FileSystemEvent::Modified).collect(),
        EventKind::Access(_) | EventKind::Any | EventKind::Other => Vec::new(),
    }
}

fn is_path_ignored(workspace_root: &Path, candidate: &Path) -> bool {
    if !candidate.starts_with(workspace_root) {
        return true;
    }

    for result in configure_walk(workspace_root, false).build() {
        let Ok(entry) = result else {
            continue;
        };
        if entry.path() == candidate {
            // The walker skips gitignored paths, so reaching `candidate` means it is not ignored.
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::thread;
    use std::time::Instant;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn walk_hides_dot_ide_by_default() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".ide").join("meta")).unwrap();
        fs::write(root.join(".ide/tasks.md"), "# Project tasks\n").unwrap();
        fs::write(root.join("src.txt"), "x\n").unwrap();

        let ws = Workspace::open(root).unwrap();
        let rels: Vec<_> = ws.walk_files().unwrap().into_iter().map(|e| e.relative).collect();
        assert!(
            rels.iter().all(|r| !relative_is_under_dot_ide(r)),
            "walk_files should hide .ide paths, got {rels:?}"
        );

        let rels_show: Vec<_> = ws
            .walk_files_with_options(WalkOptions { include_ide_internals: true })
            .unwrap()
            .into_iter()
            .filter(|e| e.kind == FileKind::Regular)
            .map(|e| e.relative)
            .collect();
        assert!(
            rels_show.iter().any(|r| r.to_string_lossy().contains("tasks.md")),
            "with include_ide_internals, expected .ide/tasks.md, got {rels_show:?}"
        );
    }

    #[test]
    fn walk_respects_gitignore() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("keep.txt"), "x\n").unwrap();
        fs::write(root.join(".gitignore"), "drop.txt\n").unwrap();
        fs::write(root.join("drop.txt"), "nope\n").unwrap();

        let ws = Workspace::open(root).unwrap();
        let files: Vec<_> = ws
            .walk_files()
            .unwrap()
            .into_iter()
            .filter(|e| e.kind == FileKind::Regular)
            .map(|e| e.path.file_name().unwrap().to_os_string())
            .collect();

        assert!(files.iter().any(|n| n == "keep.txt"));
        assert!(!files.iter().any(|n| n == "drop.txt"));
    }

    #[test]
    #[ignore = "timing-sensitive filesystem notify; run: cargo test -p editor-workspace watcher -- --ignored"]
    fn watcher_sees_external_write() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let ws = Workspace::open(&root).unwrap();
        let watchme = root.join("watch.txt");

        // Windows may need a short settle after `open` before events are reliable.
        thread::sleep(Duration::from_millis(80));
        fs::write(&watchme, "hello\n").unwrap();

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut saw = false;
        while Instant::now() < deadline {
            for ev in ws.poll_events() {
                let path = match ev {
                    FileSystemEvent::Created(ref p) | FileSystemEvent::Modified(ref p) => {
                        Some(p.as_path())
                    }
                    _ => None,
                };
                if path.is_some_and(|p| {
                    p.file_name() == watchme.file_name()
                        && p.parent().map(|x| x.as_os_str())
                            == watchme.parent().map(|x| x.as_os_str())
                }) {
                    saw = true;
                }
            }
            if saw {
                break;
            }
            thread::sleep(Duration::from_millis(30));
        }

        assert!(saw, "expected Created/Modified for new file under watched root");
    }
}
