//! Incremental text edits ([`Edit`], [`EditKind`]) for undo and [`super::TextBuffer::apply_edit`].

use std::ops::Range;

use crate::position::BytePos;
use crate::{CoreError, CoreResult};

use super::TextBuffer;

/// High-level edit operation (before sequence assignment).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditKind {
    Insert { pos: BytePos, text: String },
    Delete { range: Range<BytePos>, deleted_text: String },
}

/// One applied edit with monotonic sequence from the buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub kind: EditKind,
    pub seq: u64,
}

impl Edit {
    /// Inverse edit (undo applies this to revert `self`).
    #[must_use]
    pub fn inverse(&self) -> Edit {
        match &self.kind {
            EditKind::Insert { pos, text } => {
                let end = BytePos(pos.0 + text.len());
                Edit {
                    kind: EditKind::Delete { range: *pos..end, deleted_text: text.clone() },
                    seq: self.seq,
                }
            }
            EditKind::Delete { range, deleted_text } => Edit {
                kind: EditKind::Insert { pos: range.start, text: deleted_text.clone() },
                seq: self.seq,
            },
        }
    }

    /// Apply this edit to `buffer` (same effect as the original `apply_edit` step).
    pub fn apply(&self, buffer: &mut TextBuffer) -> CoreResult<()> {
        match &self.kind {
            EditKind::Insert { pos, text } => buffer.insert(*pos, text),
            EditKind::Delete { range, deleted_text } => {
                let actual = buffer.delete_range(range.clone())?;
                if actual != *deleted_text {
                    buffer.insert(range.start, &actual)?;
                    return Err(CoreError::InvalidRange {
                        start: range.start.0,
                        end: range.end.0,
                        len: buffer.len_bytes(),
                    });
                }
                Ok(())
            }
        }
    }
}
