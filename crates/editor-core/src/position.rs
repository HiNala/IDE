//! Position types for the text buffer.
//!
//! Byte offsets are canonical. [`LineCol`] exists for human-facing display
//! (for example the status bar) but is derived, not stored.

use std::ops::{Add, Sub};

/// A byte offset into the buffer's UTF-8 content. Values produced by this
/// crate's APIs always lie on a UTF-8 scalar boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BytePos(pub usize);

impl BytePos {
    /// Zero byte offset (start of buffer).
    pub const ZERO: Self = Self(0);
}

impl From<usize> for BytePos {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<BytePos> for usize {
    fn from(value: BytePos) -> Self {
        value.0
    }
}

impl Add<usize> for BytePos {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0.saturating_add(rhs))
    }
}

impl Sub<usize> for BytePos {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0.saturating_sub(rhs))
    }
}

/// A (line, column) pair. Column is measured in UTF-8 bytes within the line,
/// not in grapheme clusters or display width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineCol {
    /// Zero-based line index.
    pub line: usize,
    /// Column as UTF-8 byte offset from the start of the line.
    pub col: usize,
}

impl LineCol {
    /// Constructs a zero-based line/column pair (column is UTF-8 bytes in the line).
    #[must_use]
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_pos_arithmetic() {
        assert_eq!(BytePos(3) + 2, BytePos(5));
        assert_eq!(BytePos(5) - 2, BytePos(3));
        assert_eq!(usize::from(BytePos(7)), 7);
    }
}
