//! Reversible edit operations.

use std::ops::Range;

use super::TextBuffer;
use crate::position::BytePos;
use crate::CoreResult;

/// Kind of text change (insert or delete).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditKind {
    /// Insert `text` at `pos` (byte offset, on a UTF-8 boundary).
    Insert {
        /// Insertion point.
        pos: BytePos,
        /// Text to insert.
        text: String,
    },
    /// Delete `range`; `deleted_text` must match buffer contents when applied.
    Delete {
        /// Byte range to remove (on UTF-8 boundaries).
        range: Range<BytePos>,
        /// Text that was / will be removed (for redo and inverse).
        deleted_text: String,
    },
}

/// One applied edit with a monotonic sequence number from [`super::TextBuffer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// Edit payload.
    pub kind: EditKind,
    /// Sequence number assigned by the buffer.
    pub seq: u64,
}

impl Edit {
    /// Apply this edit to `buffer`. Fails if offsets or `deleted_text` do not match the buffer.
    pub fn apply(&self, buffer: &mut TextBuffer) -> CoreResult<()> {
        match &self.kind {
            EditKind::Insert { pos, text } => buffer.insert(*pos, text),
            EditKind::Delete { range, deleted_text } => {
                let actual = buffer.delete_range(range.clone())?;
                if actual != *deleted_text {
                    buffer.insert(range.start, &actual)?;
                    return Err(crate::CoreError::InvalidRange {
                        start: range.start.0,
                        end: range.end.0,
                        len: buffer.len_bytes(),
                    });
                }
                Ok(())
            }
        }
    }

    /// Inverse edit that undoes `self` when applied.
    #[must_use]
    pub fn inverse(&self) -> Self {
        let kind = match &self.kind {
            EditKind::Insert { pos, text } => EditKind::Delete {
                range: *pos..BytePos(pos.0 + text.len()),
                deleted_text: text.clone(),
            },
            EditKind::Delete { range, deleted_text } => {
                EditKind::Insert { pos: range.start, text: deleted_text.clone() }
            }
        };
        Self { kind, seq: self.seq }
    }
}
