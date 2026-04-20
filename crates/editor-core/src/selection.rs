//! Anchor/head selection (single region).

use std::ops::Range;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_ordering() {
        let s = Selection { anchor: BytePos(5), head: BytePos(2) };
        assert_eq!(s.range(), BytePos(2)..BytePos(5));
        assert_eq!(s.len_bytes(), 3);
    }
}
