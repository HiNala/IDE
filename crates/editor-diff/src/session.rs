//! Per-hunk accept / reject (M23 wiring); applies through [`TextBuffer::apply_edit`](editor_core::TextBuffer::apply_edit).

use editor_core::{BytePos, CoreResult, EditKind, TextBuffer, UndoStack};

use crate::types::Hunk;

/// Tracks which hunks were rejected before application (M23).
#[derive(Debug, Clone)]
pub struct DiffReviewState {
    pub old_snapshot: String,
    pub new_snapshot: String,
    pub hunks: Vec<Hunk>,
    pub rejected: Vec<bool>,
}

impl DiffReviewState {
    #[must_use]
    pub fn new(old_snapshot: String, new_snapshot: String, hunks: Vec<Hunk>) -> Self {
        let n = hunks.len();
        Self { old_snapshot, new_snapshot, hunks, rejected: vec![false; n] }
    }

    pub fn reject_hunk(&mut self, idx: usize) {
        if let Some(r) = self.rejected.get_mut(idx) {
            *r = true;
        }
    }

    pub fn clear_reject(&mut self, idx: usize) {
        if let Some(r) = self.rejected.get_mut(idx) {
            *r = false;
        }
    }
}

/// Applies one hunk: replaces the corresponding span in `buffer` (which must match `state.old_snapshot`)
/// with the new-side text from `state.new_snapshot`.
pub fn apply_hunk_to_buffer(
    buffer: &mut TextBuffer,
    undo: &mut UndoStack,
    state: &DiffReviewState,
    hunk_idx: usize,
) -> CoreResult<()> {
    let Some(h) = state.hunks.get(hunk_idx) else {
        return Ok(());
    };
    if state.rejected.get(hunk_idx).copied().unwrap_or(false) {
        return Ok(());
    }
    let old_buf = TextBuffer::from_str(&state.old_snapshot);
    let new_buf = TextBuffer::from_str(&state.new_snapshot);

    let ol0 = h.header.old_start.saturating_sub(1);
    let nl0 = h.header.new_start.saturating_sub(1);
    let old_start_byte = old_buf.line_to_byte(ol0)?;
    let old_end_line = ol0 + h.header.old_lines;
    let old_end_byte = if old_end_line >= old_buf.len_lines() {
        old_buf.len_bytes()
    } else {
        old_buf.line_to_byte(old_end_line)?
    };

    let new_start_byte = new_buf.line_to_byte(nl0)?;
    let new_end_line = nl0 + h.header.new_lines;
    let new_end_byte = if new_end_line >= new_buf.len_lines() {
        new_buf.len_bytes()
    } else {
        new_buf.line_to_byte(new_end_line)?
    };

    let replacement = new_buf.slice_to_string(BytePos(new_start_byte)..BytePos(new_end_byte))?;

    let current = buffer.to_text();
    if current != state.old_snapshot {
        return Err(editor_core::CoreError::InvalidEdit {
            message: "diff apply requires buffer to match diff base text",
        });
    }

    let deleted = buffer.slice_to_string(BytePos(old_start_byte)..BytePos(old_end_byte))?;
    let edit_del = buffer.apply_edit(EditKind::Delete {
        range: BytePos(old_start_byte)..BytePos(old_end_byte),
        deleted_text: deleted,
    })?;
    undo.push(edit_del);
    if !replacement.is_empty() {
        let edit_ins = buffer
            .apply_edit(EditKind::Insert { pos: BytePos(old_start_byte), text: replacement })?;
        undo.push(edit_ins);
    }
    Ok(())
}
