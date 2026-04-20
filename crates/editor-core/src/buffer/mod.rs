//! Rope-backed text buffer with byte-centric API.

pub mod edit;
pub mod line_ending;

pub use edit::{Edit, EditKind};
pub use line_ending::LineEnding;

use std::ops::Range;

use ropey::Rope;

use crate::buffer::line_ending::normalize_to_lf;
use crate::position::{BytePos, LineCol};
use crate::{CoreError, CoreResult};

/// One line of text as a cheap view over a [`ropey::RopeSlice`].
#[derive(Clone, Copy, Debug)]
pub struct RopeLineSlice<'a> {
    inner: ropey::RopeSlice<'a>,
}

impl<'a> RopeLineSlice<'a> {
    /// Length of the line in UTF-8 bytes (excluding the line terminator).
    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.inner.len_bytes()
    }

    /// Contiguous `&str` when the line fits in one rope chunk; otherwise `None`.
    #[must_use]
    pub fn as_str(&self) -> Option<&'a str> {
        self.inner.as_str()
    }

    /// Full line text (may allocate).
    #[must_use]
    pub fn to_line_string(&self) -> String {
        self.inner.to_string()
    }
}

/// Cheap snapshot of buffer content for background work (parse, save).
#[derive(Clone, Debug)]
pub struct TextBufferSnapshot {
    rope: Rope,
    version: u64,
}

impl TextBufferSnapshot {
    /// Rope clone (Arc-backed, cheap).
    #[must_use]
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// Buffer version at snapshot time.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }
}

/// UTF-8 text document stored as a rope. Internal newlines are always `'\n'`;
/// [`LineEnding`] records the on-disk convention for saves.
#[derive(Clone, Debug)]
pub struct TextBuffer {
    rope: Rope,
    original_line_ending: LineEnding,
    version: u64,
    next_edit_seq: u64,
}

impl TextBuffer {
    /// Empty buffer with LF convention.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            original_line_ending: LineEnding::Lf,
            version: 0,
            next_edit_seq: 1,
        }
    }

    /// Parse `s`, normalize to LF internally, and record detected line ending.
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        let original_line_ending = LineEnding::detect(s);
        let normalized = normalize_to_lf(s);
        Self {
            rope: Rope::from_str(&normalized),
            original_line_ending,
            version: 0,
            next_edit_seq: 1,
        }
    }

    /// Same as [`Self::from_str`], but forces the recorded original line ending
    /// (internal storage is always LF-normalized).
    #[must_use]
    pub fn with_line_ending(s: &str, le: LineEnding) -> Self {
        let normalized = normalize_to_lf(s);
        Self {
            rope: Rope::from_str(&normalized),
            original_line_ending: le,
            version: 0,
            next_edit_seq: 1,
        }
    }

    /// Byte offset of the start of `line`.
    pub fn line_to_byte(&self, line: usize) -> CoreResult<usize> {
        let total = self.rope.len_lines();
        if line >= total {
            return Err(CoreError::InvalidLineIndex { line, total_lines: total });
        }
        Ok(self.rope.line_to_byte(line))
    }

    /// UTF-8 byte length of `line` (excluding line break).
    pub fn line_len_bytes(&self, line: usize) -> CoreResult<usize> {
        let total = self.rope.len_lines();
        if line >= total {
            return Err(CoreError::InvalidLineIndex { line, total_lines: total });
        }
        Ok(self.rope.line(line).len_bytes())
    }

    /// Line ending detected or assigned at load.
    #[must_use]
    pub const fn original_line_ending(&self) -> LineEnding {
        self.original_line_ending
    }

    /// Monotonic edit counter (insert/delete bump this).
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Full document as a `String` (for tests and grapheme helpers; avoid on huge files).
    #[must_use]
    pub fn to_text(&self) -> String {
        self.rope.to_string()
    }

    /// Total length in UTF-8 bytes.
    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Number of lines (rope semantics: trailing newline adds an empty last line).
    #[must_use]
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Returns line `line` or `None` if out of range.
    #[must_use]
    pub fn line(&self, line: usize) -> Option<RopeLineSlice<'_>> {
        if line >= self.rope.len_lines() {
            return None;
        }
        Some(RopeLineSlice { inner: self.rope.line(line) })
    }

    /// Whether `pos` lies on a UTF-8 scalar boundary (or end of buffer).
    #[must_use]
    pub fn is_char_boundary(&self, pos: BytePos) -> bool {
        let b = pos.0;
        if b > self.rope.len_bytes() {
            return false;
        }
        if b == self.rope.len_bytes() {
            return true;
        }
        self.rope.char_to_byte(self.rope.byte_to_char(b)) == b
    }

    /// Convert byte offset to zero-based line and column (bytes within line).
    pub fn byte_to_line_col(&self, pos: BytePos) -> CoreResult<LineCol> {
        let b = pos.0;
        if b > self.rope.len_bytes() {
            return Err(CoreError::InvalidOffset { offset: b, len: self.rope.len_bytes() });
        }
        if !self.is_char_boundary(pos) {
            return Err(CoreError::InvalidCharBoundary { offset: b });
        }
        let line = self.rope.byte_to_line(b);
        let line_start = self.rope.line_to_byte(line);
        Ok(LineCol { line, col: b - line_start })
    }

    /// Convert line/column to absolute byte offset.
    pub fn line_col_to_byte(&self, lc: LineCol) -> CoreResult<BytePos> {
        let total = self.rope.len_lines();
        if lc.line >= total {
            return Err(CoreError::InvalidLineIndex { line: lc.line, total_lines: total });
        }
        let line_start = self.rope.line_to_byte(lc.line);
        let line_slice = self.rope.line(lc.line);
        let max_col = line_slice.len_bytes();
        if lc.col > max_col {
            return Err(CoreError::InvalidColumn { line: lc.line, col: lc.col, max_col });
        }
        let byte = line_start + lc.col;
        if !self.is_char_boundary(BytePos(byte)) {
            return Err(CoreError::InvalidCharBoundary { offset: byte });
        }
        Ok(BytePos(byte))
    }

    /// Copy `range` into a new [`String`].
    pub fn slice_to_string(&self, range: Range<BytePos>) -> CoreResult<String> {
        let start = range.start.0;
        let end = range.end.0;
        self.validate_range(start, end)?;
        let c_start = self.rope.byte_to_char(start);
        let c_end = self.rope.byte_to_char(end);
        Ok(self.rope.slice(c_start..c_end).to_string())
    }

    /// Iterator over internal rope chunks with absolute byte offsets.
    pub fn chunks_with_offsets(&self) -> impl Iterator<Item = (BytePos, &str)> + '_ {
        let mut off = 0usize;
        self.rope.chunks().map(move |s| {
            let p = BytePos(off);
            off += s.len();
            (p, s)
        })
    }

    /// Cheap content snapshot.
    #[must_use]
    pub fn snapshot(&self) -> TextBufferSnapshot {
        TextBufferSnapshot { rope: self.rope.clone(), version: self.version }
    }

    /// Insert `text` at `pos`.
    pub fn insert(&mut self, pos: BytePos, text: &str) -> CoreResult<()> {
        let b = pos.0;
        if b > self.rope.len_bytes() {
            return Err(CoreError::InvalidOffset { offset: b, len: self.rope.len_bytes() });
        }
        if !self.is_char_boundary(pos) {
            return Err(CoreError::InvalidCharBoundary { offset: b });
        }
        let c = self.rope.byte_to_char(b);
        self.rope.insert(c, text);
        self.version = self.version.wrapping_add(1);
        Ok(())
    }

    /// Delete `range` and return the removed text.
    pub fn delete_range(&mut self, range: Range<BytePos>) -> CoreResult<String> {
        let start = range.start.0;
        let end = range.end.0;
        self.validate_range(start, end)?;
        let c_start = self.rope.byte_to_char(start);
        let c_end = self.rope.byte_to_char(end);
        let deleted = self.rope.slice(c_start..c_end).to_string();
        self.rope.remove(c_start..c_end);
        self.version = self.version.wrapping_add(1);
        Ok(deleted)
    }

    /// Apply an [`EditKind`], assign sequence number, bump version.
    pub fn apply_edit(&mut self, kind: EditKind) -> CoreResult<Edit> {
        let seq = self.next_edit_seq;
        self.next_edit_seq += 1;
        match kind {
            EditKind::Insert { pos, text } => {
                self.insert(pos, &text)?;
                Ok(Edit { kind: EditKind::Insert { pos, text }, seq })
            }
            EditKind::Delete { range, deleted_text } => {
                let actual = self.delete_range(range.clone())?;
                if actual != deleted_text {
                    self.insert(range.start, &actual)?;
                    return Err(CoreError::InvalidRange {
                        start: range.start.0,
                        end: range.end.0,
                        len: self.len_bytes(),
                    });
                }
                Ok(Edit { kind: EditKind::Delete { range, deleted_text: actual }, seq })
            }
        }
    }

    fn validate_range(&self, start: usize, end: usize) -> CoreResult<()> {
        let len = self.rope.len_bytes();
        if start > end {
            return Err(CoreError::InvalidRange { start, end, len });
        }
        if end > len {
            return Err(CoreError::InvalidOffset { offset: end, len });
        }
        if !self.is_char_boundary(BytePos(start)) {
            return Err(CoreError::InvalidCharBoundary { offset: start });
        }
        if !self.is_char_boundary(BytePos(end)) {
            return Err(CoreError::InvalidCharBoundary { offset: end });
        }
        Ok(())
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_delete_ascii() {
        let mut b = TextBuffer::new();
        b.insert(BytePos(0), "hello").unwrap();
        assert_eq!(b.len_bytes(), 5);
        let del = b.delete_range(BytePos(1)..BytePos(4)).unwrap();
        assert_eq!(del, "ell");
        assert_eq!(b.slice_to_string(BytePos(0)..BytePos(2)).unwrap(), "ho");
    }

    #[test]
    fn byte_line_col_round_trip() {
        let b = TextBuffer::from_str("a\nb\n");
        let p = b.line_col_to_byte(LineCol { line: 1, col: 0 }).unwrap();
        assert_eq!(p.0, 2);
        assert_eq!(b.byte_to_line_col(p).unwrap(), LineCol { line: 1, col: 0 });
    }

    #[test]
    fn version_bumps() {
        let mut b = TextBuffer::new();
        assert_eq!(b.version(), 0);
        b.insert(BytePos(0), "x").unwrap();
        assert_eq!(b.version(), 1);
    }
}
