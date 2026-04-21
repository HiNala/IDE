//! Multi-buffer bookkeeping (M13+). Each [`BufferId`] maps to a full editor [`BufferState`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crossbeam_channel::Receiver;

use editor_core::{BytePos, Cursor, ScrollOffset, Selection, TextBuffer, UndoStack, WorkerPool};
use editor_io::{
    load_file_async, save_file_sync, Encoding, LoadError, LoadProgress, LoadedFile, SaveError,
};

/// Stable handle for an open document tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(pub u64);

/// In-memory editor state for one buffer (cursor, selection, undo, scroll, path).
#[derive(Debug)]
pub struct BufferState {
    pub buffer: TextBuffer,
    pub cursor: Cursor,
    pub selection: Selection,
    pub undo: UndoStack,
    pub scroll: ScrollOffset,
    pub path: Option<PathBuf>,
    pub disk_encoding: Encoding,
    pub dirty: bool,
    pub external_modified: bool,
    pub file_mtime: Option<SystemTime>,
}

impl BufferState {
    /// Empty untitled buffer (UTF-8, LF).
    #[must_use]
    pub fn new_empty() -> Self {
        Self::new_empty_coalesced(editor_core::undo::COALESCE_MS)
    }

    #[must_use]
    pub fn new_empty_coalesced(undo_coalesce_ms: u64) -> Self {
        let z = BytePos(0);
        Self {
            buffer: TextBuffer::new(),
            cursor: Cursor::new(z),
            selection: Selection::empty(z),
            undo: UndoStack::new(10_000, Duration::from_millis(undo_coalesce_ms)),
            scroll: ScrollOffset::default(),
            path: None,
            disk_encoding: Encoding::Utf8,
            dirty: false,
            external_modified: false,
            file_mtime: None,
        }
    }

    /// State after loading a file from disk (caret at start; scroll reset).
    #[must_use]
    pub fn from_loaded(l: LoadedFile) -> Self {
        Self::from_loaded_coalesced(l, editor_core::undo::COALESCE_MS)
    }

    #[must_use]
    pub fn from_loaded_coalesced(l: LoadedFile, undo_coalesce_ms: u64) -> Self {
        let z = BytePos(0);
        Self {
            buffer: l.buffer,
            cursor: Cursor::new(z),
            selection: Selection::empty(z),
            undo: UndoStack::new(10_000, Duration::from_millis(undo_coalesce_ms)),
            scroll: ScrollOffset::default(),
            path: Some(l.path),
            disk_encoding: l.encoding,
            dirty: false,
            external_modified: false,
            file_mtime: Some(l.mtime),
        }
    }
}

/// Attempted to close a buffer that is not tracked, or close with unsaved edits.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CloseError {
    #[error("buffer not found")]
    NotFound,
    #[error("buffer has unsaved changes")]
    UnsavedChanges,
}

/// Owns multiple [`BufferState`] entries; MRU ordering for Ctrl+Tab cycling.
#[derive(Debug)]
pub struct BufferManager {
    next_id: u64,
    buffers: HashMap<BufferId, BufferState>,
    /// Most-recent first (matches future tab strip).
    order: Vec<BufferId>,
    active: Option<BufferId>,
}

impl Default for BufferManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferManager {
    #[must_use]
    pub fn new() -> Self {
        Self { next_id: 1, buffers: HashMap::new(), order: Vec::new(), active: None }
    }

    fn alloc_id(&mut self) -> BufferId {
        let id = BufferId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    fn touch_mru(&mut self, id: BufferId) {
        self.order.retain(|&x| x != id);
        self.order.insert(0, id);
    }

    /// New empty buffer, becomes active.
    pub fn create_empty(&mut self) -> BufferId {
        self.create_empty_coalesced(editor_core::undo::COALESCE_MS)
    }

    /// Alias for mission docs / API symmetry (`new_untitled`).
    pub fn new_untitled(&mut self) -> BufferId {
        self.create_empty()
    }

    pub fn new_untitled_coalesced(&mut self, undo_coalesce_ms: u64) -> BufferId {
        self.create_empty_coalesced(undo_coalesce_ms)
    }

    pub fn create_empty_coalesced(&mut self, undo_coalesce_ms: u64) -> BufferId {
        let id = self.alloc_id();
        self.buffers.insert(id, BufferState::new_empty_coalesced(undo_coalesce_ms));
        self.active = Some(id);
        self.touch_mru(id);
        id
    }

    #[must_use]
    pub fn same_path(a: &Path, b: &Path) -> bool {
        match (a.canonicalize(), b.canonicalize()) {
            (Ok(x), Ok(y)) => x == y,
            _ => a == b,
        }
    }

    #[must_use]
    pub fn find_by_path(&self, path: &Path) -> Option<BufferId> {
        for (id, st) in &self.buffers {
            let Some(ref p) = st.path else {
                continue;
            };
            if Self::same_path(p, path) {
                return Some(*id);
            }
        }
        None
    }

    /// Load UTF-8 text from disk; becomes active. Returns existing tab if `path` is already open.
    pub fn open_file(&mut self, path: &Path) -> Result<BufferId, LoadError> {
        self.open_file_coalesced(path, editor_core::undo::COALESCE_MS)
    }

    pub fn open_file_coalesced(
        &mut self,
        path: &Path,
        undo_coalesce_ms: u64,
    ) -> Result<BufferId, LoadError> {
        if let Some(id) = self.find_by_path(path) {
            self.switch_to(id).expect("find_by_path implies buffer exists");
            return Ok(id);
        }
        let loaded = editor_io::load_file_sync(path)?;
        let id = self.alloc_id();
        let state = BufferState::from_loaded_coalesced(loaded, undo_coalesce_ms);
        self.buffers.insert(id, state);
        self.active = Some(id);
        self.touch_mru(id);
        Ok(id)
    }

    /// Open a new tab with an empty [`TextBuffer`] and kick off background load via [`load_file_async`].
    ///
    /// Returns `(id, None)` when the path is already open (switches to that tab). Otherwise
    /// `(id, Some(rx))` to poll [`LoadProgress`] on the main thread.
    pub fn open_file_async_start(
        &mut self,
        path: PathBuf,
        pool: &WorkerPool,
        undo_coalesce_ms: u64,
    ) -> (BufferId, Option<Receiver<LoadProgress>>) {
        if let Some(id) = self.find_by_path(&path) {
            let _ = self.switch_to(id);
            return (id, None);
        }
        let id = self.alloc_id();
        let mut st = BufferState::new_empty_coalesced(undo_coalesce_ms);
        st.path = Some(path.clone());
        self.buffers.insert(id, st);
        self.active = Some(id);
        self.touch_mru(id);
        let (_, rx) = load_file_async(pool, path);
        (id, Some(rx))
    }

    /// Replace an existing buffer's state with a loaded file (e.g. deferred async load completed).
    pub fn replace_with_loaded(&mut self, id: BufferId, l: LoadedFile) -> Result<(), CloseError> {
        self.replace_with_loaded_coalesced(id, l, editor_core::undo::COALESCE_MS)
    }

    pub fn replace_with_loaded_coalesced(
        &mut self,
        id: BufferId,
        l: LoadedFile,
        undo_coalesce_ms: u64,
    ) -> Result<(), CloseError> {
        if !self.buffers.contains_key(&id) {
            return Err(CloseError::NotFound);
        }
        self.buffers.insert(id, BufferState::from_loaded_coalesced(l, undo_coalesce_ms));
        self.touch_mru(id);
        Ok(())
    }

    /// Insert a fully loaded file as a new tab (e.g. initial CLI open).
    pub fn push_loaded(&mut self, l: LoadedFile) -> BufferId {
        self.push_loaded_coalesced(l, editor_core::undo::COALESCE_MS)
    }

    pub fn push_loaded_coalesced(&mut self, l: LoadedFile, undo_coalesce_ms: u64) -> BufferId {
        if let Some(id) = self.find_by_path(&l.path) {
            self.switch_to(id).expect("find_by_path implies buffer exists");
            return id;
        }
        let id = self.alloc_id();
        self.buffers.insert(id, BufferState::from_loaded_coalesced(l, undo_coalesce_ms));
        self.active = Some(id);
        self.touch_mru(id);
        id
    }

    #[must_use]
    pub fn get(&self, id: BufferId) -> Option<&BufferState> {
        self.buffers.get(&id)
    }

    #[must_use]
    pub fn get_mut(&mut self, id: BufferId) -> Option<&mut BufferState> {
        self.buffers.get_mut(&id)
    }

    #[must_use]
    pub fn active(&self) -> Option<BufferId> {
        self.active
    }

    /// Active buffer state, or `None` when empty.
    #[must_use]
    pub fn active_ref(&self) -> Option<&BufferState> {
        let id = self.active?;
        self.buffers.get(&id)
    }

    /// Active buffer state (mutable).
    pub fn active_mut(&mut self) -> Option<&mut BufferState> {
        let id = self.active?;
        self.buffers.get_mut(&id)
    }

    /// Tab strip order: oldest buffer first (left). Internal [`Self::order`] is MRU-first.
    #[must_use]
    pub fn order_oldest_first(&self) -> Vec<BufferId> {
        self.order.iter().rev().copied().collect()
    }

    pub fn switch_to(&mut self, id: BufferId) -> Result<(), CloseError> {
        if !self.buffers.contains_key(&id) {
            return Err(CloseError::NotFound);
        }
        self.active = Some(id);
        self.touch_mru(id);
        Ok(())
    }

    /// Cycle MRU: next tab (wrap). **Ctrl+Tab** in the app shell.
    pub fn next_buffer(&mut self) {
        let Some(active) = self.active else {
            return;
        };
        let n = self.order.len();
        if n <= 1 {
            return;
        }
        let Some(pos) = self.order.iter().position(|&x| x == active) else {
            return;
        };
        let next = self.order[(pos + 1) % n];
        let _ = self.switch_to(next);
    }

    /// Sync-save to disk. `path == None` uses the tab's existing path (error if untitled).
    /// `Some(path)` performs Save As and updates the buffer path.
    pub fn save(&mut self, id: BufferId, path: Option<PathBuf>) -> Result<(), SaveError> {
        let state = self
            .buffers
            .get_mut(&id)
            .ok_or_else(|| SaveError::Io(std::io::Error::other("save: buffer id not found")))?;
        let target = path.or_else(|| state.path.clone()).ok_or_else(|| {
            SaveError::Io(std::io::Error::other(
                "save: untitled buffer — pass a path or set buffer path first",
            ))
        })?;
        let snapshot = state.buffer.snapshot();
        let le = state.buffer.original_line_ending();
        let enc = state.disk_encoding;
        save_file_sync(&target, &snapshot, le, enc)?;
        state.path = Some(target.clone());
        state.dirty = false;
        state.external_modified = false;
        state.file_mtime = std::fs::metadata(&target).and_then(|m| m.modified()).ok();
        self.touch_mru(id);
        Ok(())
    }

    /// Cycle MRU: previous tab (wrap). **Ctrl+Shift+Tab**.
    pub fn prev_buffer(&mut self) {
        let Some(active) = self.active else {
            return;
        };
        let n = self.order.len();
        if n <= 1 {
            return;
        }
        let Some(pos) = self.order.iter().position(|&x| x == active) else {
            return;
        };
        let prev = self.order[(pos + n - 1) % n];
        let _ = self.switch_to(prev);
    }

    /// Update on-disk path when the OS reports a rename.
    pub fn rename_buffer_path(&mut self, from: &Path, to: &Path) {
        for st in self.buffers.values_mut() {
            let Some(ref p) = st.path else {
                continue;
            };
            if Self::same_path(p, from) {
                st.path = Some(to.to_path_buf());
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (BufferId, &BufferState)> + '_ {
        self.buffers.iter().map(|(&id, st)| (id, st))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (BufferId, &mut BufferState)> + '_ {
        self.buffers.iter_mut().map(|(&id, st)| (id, st))
    }

    /// Remove a buffer. With `force == false`, refuses when [`BufferState::dirty`].
    pub fn close(&mut self, id: BufferId, force: bool) -> Result<(), CloseError> {
        let Some(st) = self.buffers.get(&id) else {
            return Err(CloseError::NotFound);
        };
        if st.dirty && !force {
            return Err(CloseError::UnsavedChanges);
        }
        self.buffers.remove(&id);
        self.order.retain(|&x| x != id);
        if self.active == Some(id) {
            self.active = self.order.first().copied();
        }
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// 1-based index of the active buffer in the MRU [`Self::order`] (same order as Ctrl+Tab), and buffer count.
    #[must_use]
    pub fn active_mru_index(&self) -> Option<(usize, usize)> {
        let n = self.order.len();
        if n == 0 {
            return None;
        }
        let id = self.active?;
        let pos = self.order.iter().position(|&x| x == id)?;
        Some((pos + 1, n))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    /// UTF-8 text for dirty buffers that have a path (project search must not read stale disk).
    #[must_use]
    pub fn dirty_path_contents(&self) -> HashMap<PathBuf, String> {
        self.buffers
            .values()
            .filter_map(|st| {
                if !st.dirty {
                    return None;
                }
                let p = st.path.as_ref()?.clone();
                Some((p, st.buffer.to_text()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn create_empty_sets_active() {
        let mut m = BufferManager::new();
        let id = m.create_empty();
        assert_eq!(m.active(), Some(id));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn open_file_loads_text() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "hello").unwrap();
        let mut m = BufferManager::new();
        let id = m.open_file(tmp.path()).unwrap();
        assert_eq!(m.get(id).unwrap().buffer.to_text(), "hello\n");
    }

    #[test]
    fn open_same_path_twice_reuses_tab() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "x").unwrap();
        let mut m = BufferManager::new();
        let a = m.open_file(tmp.path()).unwrap();
        m.create_empty();
        let b = m.open_file(tmp.path()).unwrap();
        assert_eq!(a, b);
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn next_prev_cycles() {
        let mut m = BufferManager::new();
        let _ = m.create_empty();
        let _ = m.create_empty();
        let start = m.active().unwrap();
        m.next_buffer();
        assert_ne!(m.active(), Some(start));
        m.prev_buffer();
        assert_eq!(m.active(), Some(start));
    }

    #[test]
    fn active_mru_index_matches_order() {
        let mut m = BufferManager::new();
        let _ = m.create_empty();
        let _ = m.create_empty();
        assert_eq!(m.active_mru_index(), Some((1, 2)));
    }

    #[test]
    fn close_refuses_dirty_without_force() {
        let mut m = BufferManager::new();
        let id = m.create_empty();
        m.get_mut(id).unwrap().dirty = true;
        assert!(matches!(m.close(id, false), Err(CloseError::UnsavedChanges)));
        assert!(m.close(id, true).is_ok());
    }
}
