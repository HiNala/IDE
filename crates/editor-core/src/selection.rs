//! Anchor/head selection (single region).

use std::ops::Range;

use crate::buffer::TextBuffer;
use crate::position::BytePos;

/// A text selection: anchor is where the drag started; head is the caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Selection start (byte offset on a UTF-8 boundary).
    pub anchor: BytePos,
    /// Active end (caret).
    pub head: BytePos,
}

impl Selection {
    /// Collapsed selection at `pos`.
    #[must_use]
    pub const fn empty(pos: BytePos) -> Self {
        Self { anchor: pos, head: pos }
    }

    /// Whether anchor and head coincide.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.anchor == self.head
    }

    /// Inclusive-min, exclusive-max byte range covering the selection.
    #[must_use]
    pub fn range(self) -> Range<BytePos> {
        let a = self.anchor.0;
        let b = self.head.0;
        if a <= b {
            BytePos(a)..BytePos(b)
        } else {
            BytePos(b)..BytePos(a)
        }
    }

    /// Selection length in bytes.
    #[must_use]
    pub fn len_bytes(self) -> usize {
        let r = self.range();
        r.end.0 - r.start.0
    }

    /// Move the head to `new_head` (extends or shrinks the selection).
    pub fn extend_to(&mut self, new_head: BytePos) {
        self.head = new_head;
    }

    /// Collapse to the head side.
    pub fn collapse_to_head(&mut self) {
        self.anchor = self.head;
    }

    /// Collapse to the anchor side.
    pub fn collapse_to_anchor(&mut self) {
        self.head = self.anchor;
    }

    /// `true` when the caret (`head`) is at or after `anchor` in document order.
    #[must_use]
    pub fn is_forward(self) -> bool {
        self.head.0 >= self.anchor.0
    }

    /// Swap anchor and head (same covered range, opposite direction).
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.anchor, &mut self.head);
    }

    /// Whether `pos` lies strictly inside `[min(anchor,head), max(anchor,head))` in bytes.
    #[must_use]
    pub fn contains(self, pos: BytePos) -> bool {
        let r = self.range();
        pos.0 >= r.start.0 && pos.0 < r.end.0
    }

    /// First and last **zero-based** line indices touched by this selection (inclusive).
    #[must_use]
    pub fn line_range_inclusive(self, buffer: &TextBuffer) -> Option<(usize, usize)> {
        if self.is_empty() {
            return None;
        }
        let r = self.range();
        let first = buffer.byte_to_line_col(r.start).ok()?.line;
        let last_byte = if r.end.0 > r.start.0 { BytePos(r.end.0 - 1) } else { r.start };
        let last = buffer.byte_to_line_col(last_byte).ok()?.line;
        Some((first.min(last), first.max(last)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextBuffer;

    #[test]
    fn range_ordering() {
        let s = Selection { anchor: BytePos(5), head: BytePos(2) };
        assert_eq!(s.range(), BytePos(2)..BytePos(5));
        assert_eq!(s.len_bytes(), 3);
    }

    #[test]
    fn contains_and_forward() {
        let s = Selection { anchor: BytePos(1), head: BytePos(4) };
        assert!(s.is_forward());
        assert!(s.contains(BytePos(2)));
        assert!(!s.contains(BytePos(4)));
    }

    #[test]
    fn line_range_inclusive_multi_line() {
        let buf = TextBuffer::from_str("a\nb\nc\n");
        let s = Selection { anchor: BytePos(0), head: BytePos(4) };
        assert_eq!(s.line_range_inclusive(&buf), Some((0, 1)));
    }
}
