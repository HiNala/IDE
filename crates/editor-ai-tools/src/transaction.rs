//! Staged workspace changes: path-safe, commit via [`TextBuffer::apply_edit`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use editor_core::buffer::EditKind;
use editor_core::position::BytePos;
use editor_core::TextBuffer;
use editor_core::UndoStack;
use editor_diff::compute_line_diff;
use editor_diff::Hunk;
use editor_io::{save_file_sync, Encoding};
use editor_workspace::BufferManager;

use crate::error::{Result, ToolError};
use crate::path::canonical_under_workspace;

/// One logical edit staged against a buffer (UTF-8 byte offsets, LF-normalized buffer text).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BufferEdit {
    ReplaceRange { start_byte: usize, end_byte: usize, new_text: String },
    InsertAt { byte_offset: usize, text: String },
    FullReplace { new_text: String },
}

/// A pending filesystem / buffer mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingChange {
    EditBuffer {
        path: PathBuf,
        edit: BufferEdit,
    },
    WriteNewFile {
        path: PathBuf,
        contents: String,
    },
    /// Create or overwrite a file (used for `.ide/` metadata and task list).
    UpsertFile {
        path: PathBuf,
        contents: String,
    },
    DeleteFile {
        path: PathBuf,
        prior_contents: Option<String>,
    },
    MoveFile {
        from: PathBuf,
        to: PathBuf,
    },
}

/// Transaction scoped to one workspace root and shared [`BufferManager`].
#[derive(Debug)]
pub struct WorkspaceTx {
    root: PathBuf,
    buffers: Arc<RwLock<BufferManager>>,
    pending: Vec<PendingChange>,
}

impl WorkspaceTx {
    /// `workspace_root` should be canonical (for example from [`editor_workspace::Workspace::root`]).
    pub fn new(workspace_root: PathBuf, buffers: Arc<RwLock<BufferManager>>) -> Self {
        let root = workspace_root.canonicalize().unwrap_or(workspace_root);
        Self { root, buffers, pending: Vec::new() }
    }

    #[must_use]
    pub fn workspace_root(&self) -> &Path {
        &self.root
    }

    /// Resolve a relative path and ensure it stays under the workspace root.
    pub fn canonical_path(&self, relative: &str) -> Result<PathBuf> {
        canonical_under_workspace(&self.root, relative)
    }

    pub fn stage_change(&mut self, change: PendingChange) {
        self.pending.push(change);
    }

    #[must_use]
    pub fn pending_changes(&self) -> &[PendingChange] {
        &self.pending
    }

    pub fn rollback_all(&mut self) {
        self.pending.clear();
    }

    /// Apply pending edits with undo recording per [`EditKind`].
    pub fn commit_all(&mut self) -> Result<()> {
        let list = std::mem::take(&mut self.pending);
        for c in list {
            self.apply_one(c)?;
        }
        Ok(())
    }

    /// Commit only pending entries whose **indices** in the original `pending` list are selected.
    pub fn commit_selected(&mut self, indices: &[usize]) -> Result<()> {
        if indices.is_empty() {
            return Ok(());
        }
        let mut sorted: Vec<usize> = indices.to_vec();
        sorted.sort_unstable();
        let mut selected = vec![false; self.pending.len()];
        for i in sorted {
            if i < self.pending.len() {
                selected[i] = true;
            }
        }
        let mut kept = Vec::new();
        let mut to_apply = Vec::new();
        for (i, p) in self.pending.drain(..).enumerate() {
            if selected.get(i).copied().unwrap_or(false) {
                to_apply.push(p);
            } else {
                kept.push(p);
            }
        }
        self.pending = kept;
        for c in to_apply {
            self.apply_one(c)?;
        }
        Ok(())
    }

    fn apply_one(&mut self, change: PendingChange) -> Result<()> {
        match change {
            PendingChange::EditBuffer { path, edit } => {
                let mut mgr = self
                    .buffers
                    .write()
                    .map_err(|e| ToolError::msg(format!("buffer lock poisoned: {e}")))?;
                let id =
                    if let Some(id) = mgr.find_by_path(&path) { id } else { mgr.open_file(&path)? };
                let st = mgr.get_mut(id).ok_or(ToolError::BufferMissing)?;
                st.undo.checkpoint();
                apply_edit_to_state(&mut st.buffer, &mut st.undo, edit)?;
                st.dirty = true;
                Ok(())
            }
            PendingChange::WriteNewFile { path, contents } => {
                if path.exists() {
                    return Err(ToolError::msg(format!("create_file: {} exists", path.display())));
                }
                let buf = TextBuffer::from_str(&contents);
                save_file_sync(&path, &buf.snapshot(), buf.original_line_ending(), Encoding::Utf8)?;
                let mut mgr = self
                    .buffers
                    .write()
                    .map_err(|e| ToolError::msg(format!("buffer lock poisoned: {e}")))?;
                if let Some(id) = mgr.find_by_path(&path) {
                    let st = mgr.get_mut(id).ok_or(ToolError::BufferMissing)?;
                    st.buffer = TextBuffer::from_str(&contents);
                    st.dirty = true;
                }
                Ok(())
            }
            PendingChange::UpsertFile { path, contents } => {
                if let Some(p) = path.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let buf = TextBuffer::from_str(&contents);
                save_file_sync(&path, &buf.snapshot(), buf.original_line_ending(), Encoding::Utf8)?;
                let mut mgr = self
                    .buffers
                    .write()
                    .map_err(|e| ToolError::msg(format!("buffer lock poisoned: {e}")))?;
                if let Some(id) = mgr.find_by_path(&path) {
                    let st = mgr.get_mut(id).ok_or(ToolError::BufferMissing)?;
                    st.buffer = TextBuffer::from_str(&contents);
                    st.dirty = true;
                }
                Ok(())
            }
            PendingChange::DeleteFile { path, prior_contents: _ } => {
                std::fs::remove_file(&path)?;
                let mut mgr = self
                    .buffers
                    .write()
                    .map_err(|e| ToolError::msg(format!("buffer lock poisoned: {e}")))?;
                if let Some(id) = mgr.find_by_path(&path) {
                    mgr.close(id, true)
                        .map_err(|e| ToolError::msg(format!("close buffer: {e:?}")))?;
                }
                Ok(())
            }
            PendingChange::MoveFile { from, to } => {
                if to.exists() {
                    return Err(ToolError::msg(format!(
                        "move_file: destination {} already exists",
                        to.display()
                    )));
                }
                std::fs::rename(&from, &to)?;
                let mut mgr = self
                    .buffers
                    .write()
                    .map_err(|e| ToolError::msg(format!("buffer lock poisoned: {e}")))?;
                if let Some(id) = mgr.find_by_path(&from) {
                    if mgr.find_by_path(&to).is_some() {
                        return Err(ToolError::msg(
                            "move_file: open buffer conflict at destination",
                        ));
                    }
                    let st = mgr.get_mut(id).ok_or(ToolError::BufferMissing)?;
                    st.path = Some(to.clone());
                    st.file_mtime = std::fs::metadata(&to).and_then(|m| m.modified()).ok();
                }
                Ok(())
            }
        }
    }

    /// Line-level hunks per path for inline diff / approval UI (M23).
    pub fn preview_as_diffs(&self) -> Result<Vec<(PathBuf, Vec<Hunk>)>> {
        let mut by_path: HashMap<PathBuf, Vec<BufferEdit>> = HashMap::new();
        let mut order: Vec<PathBuf> = Vec::new();
        fn touch(order: &mut Vec<PathBuf>, p: &Path) {
            if !order.iter().any(|x| x == p) {
                order.push(p.to_path_buf());
            }
        }

        for p in &self.pending {
            match p {
                PendingChange::EditBuffer { path, edit } => {
                    touch(&mut order, path);
                    by_path.entry(path.clone()).or_default().push(edit.clone());
                }
                PendingChange::WriteNewFile { path, contents }
                | PendingChange::UpsertFile { path, contents } => {
                    touch(&mut order, path);
                    by_path
                        .entry(path.clone())
                        .or_default()
                        .push(BufferEdit::FullReplace { new_text: contents.clone() });
                }
                PendingChange::DeleteFile { path, prior_contents: _ } => {
                    touch(&mut order, path);
                    by_path
                        .entry(path.clone())
                        .or_default()
                        .push(BufferEdit::FullReplace { new_text: String::new() });
                }
                PendingChange::MoveFile { from, to } => {
                    touch(&mut order, from);
                    touch(&mut order, to);
                    let content = self.read_path_text(from).unwrap_or_default();
                    by_path
                        .entry(from.clone())
                        .or_default()
                        .push(BufferEdit::FullReplace { new_text: String::new() });
                    by_path
                        .entry(to.clone())
                        .or_default()
                        .push(BufferEdit::FullReplace { new_text: content });
                }
            }
        }
        let mut out = Vec::new();
        for path in order {
            let edits = by_path.get(&path).cloned().unwrap_or_default();
            let before = self.read_path_text(&path).unwrap_or_default();
            let mut after = before.clone();
            for e in &edits {
                apply_edit_to_string(&mut after, e)?;
            }
            let hunks = compute_line_diff(&before, &after);
            out.push((path, hunks));
        }
        Ok(out)
    }

    pub(crate) fn read_path_text(&self, path: &Path) -> Result<String> {
        let mgr = self
            .buffers
            .read()
            .map_err(|e| ToolError::msg(format!("buffer lock poisoned: {e}")))?;
        if let Some(id) = mgr.find_by_path(path) {
            return Ok(mgr.get(id).ok_or(ToolError::BufferMissing)?.buffer.to_text());
        }
        drop(mgr);
        Ok(std::fs::read_to_string(path)?)
    }

    /// Like [`Self::read_path_text`], but returns an empty string when the path is not on disk yet
    /// (staged edits against a new file).
    pub(crate) fn read_text_staging_base(&self, path: &Path) -> Result<String> {
        let mgr = self
            .buffers
            .read()
            .map_err(|e| ToolError::msg(format!("buffer lock poisoned: {e}")))?;
        if let Some(id) = mgr.find_by_path(path) {
            return Ok(mgr.get(id).ok_or(ToolError::BufferMissing)?.buffer.to_text());
        }
        drop(mgr);
        if path.exists() {
            return Ok(std::fs::read_to_string(path)?);
        }
        Ok(String::new())
    }
}

fn apply_edit_to_state(buf: &mut TextBuffer, undo: &mut UndoStack, edit: BufferEdit) -> Result<()> {
    match edit {
        BufferEdit::ReplaceRange { start_byte, end_byte, new_text } => {
            let range = BytePos(start_byte)..BytePos(end_byte);
            let deleted = buf.slice_to_string(range.clone())?;
            let e1 =
                buf.apply_edit(EditKind::Delete { range: range.clone(), deleted_text: deleted })?;
            undo.push(e1);
            let e2 =
                buf.apply_edit(EditKind::Insert { pos: BytePos(start_byte), text: new_text })?;
            undo.push(e2);
            Ok(())
        }
        BufferEdit::InsertAt { byte_offset, text } => {
            let e = buf.apply_edit(EditKind::Insert { pos: BytePos(byte_offset), text })?;
            undo.push(e);
            Ok(())
        }
        BufferEdit::FullReplace { new_text } => {
            let len = buf.len_bytes();
            let old = buf.slice_to_string(BytePos(0)..BytePos(len))?;
            let e1 = buf.apply_edit(EditKind::Delete {
                range: BytePos(0)..BytePos(len),
                deleted_text: old,
            })?;
            undo.push(e1);
            let e2 = buf.apply_edit(EditKind::Insert { pos: BytePos(0), text: new_text })?;
            undo.push(e2);
            Ok(())
        }
    }
}

fn apply_edit_to_string(s: &mut String, e: &BufferEdit) -> Result<()> {
    match e {
        BufferEdit::ReplaceRange { start_byte, end_byte, new_text } => {
            if !s.is_char_boundary(*start_byte) || !s.is_char_boundary(*end_byte) {
                return Err(ToolError::msg("edit byte offsets not on UTF-8 boundaries"));
            }
            s.replace_range(*start_byte..*end_byte, new_text.as_str());
        }
        BufferEdit::InsertAt { byte_offset, text } => {
            if !s.is_char_boundary(*byte_offset) {
                return Err(ToolError::msg("insert offset not on UTF-8 boundary"));
            }
            s.insert_str(*byte_offset, text.as_str());
        }
        BufferEdit::FullReplace { new_text } => {
            *s = new_text.clone();
        }
    }
    Ok(())
}
