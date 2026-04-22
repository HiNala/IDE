//! Primary caret with grapheme-aware motion and preferred column for vertical moves.

use unicode_segmentation::UnicodeSegmentation;

use crate::buffer::TextBuffer;
use crate::position::{BytePos, LineCol};
use crate::{CoreError, CoreResult};

/// Primary insertion caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pos: BytePos,
    /// Preferred UTF-8 column within a line for [`CursorMotion::Up`] / [`CursorMotion::Down`].
    preferred_col: Option<usize>,
}

/// Movement commands for [`Cursor::apply`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMotion {
    /// Previous grapheme cluster.
    Left,
    /// Next grapheme cluster.
    Right,
    /// One line up (uses preferred column).
    Up,
    /// One line down (uses preferred column).
    Down,
    /// Start of current line.
    LineStart,
    /// End of current line (before newline, or buffer end).
    LineEnd,
    /// Byte offset 0.
    BufferStart,
    /// End of buffer (past last byte).
    BufferEnd,
    /// Previous Unicode word boundary (see [`crate::word_nav`]).
    WordLeft,
    /// Next Unicode word boundary (see [`crate::word_nav`]).
    WordRight,
    /// Jump to an explicit boundary (must be valid).
    ByteOffset(BytePos),
}

impl Cursor {
    /// Caret at `pos`.
    #[must_use]
    pub const fn new(pos: BytePos) -> Self {
        Self { pos, preferred_col: None }
    }

    /// Current byte position.
    #[must_use]
    pub const fn pos(self) -> BytePos {
        self.pos
    }

    /// Apply `motion` against `buffer`, updating preferred column when moving horizontally.
    ///
    /// Uses a full-string grapheme pass for correctness (optimize with rope chunks in a later mission).
    pub fn apply(&mut self, motion: CursorMotion, buffer: &TextBuffer) -> CoreResult<()> {
        let s = buffer.to_text();
        match motion {
            CursorMotion::ByteOffset(p) => {
                if p.0 > buffer.len_bytes() {
                    return Err(CoreError::InvalidOffset { offset: p.0, len: buffer.len_bytes() });
                }
                if !buffer.is_char_boundary(p) {
                    return Err(CoreError::InvalidCharBoundary { offset: p.0 });
                }
                self.pos = p;
                self.refresh_preferred_col(buffer)?;
            }
            CursorMotion::BufferStart => {
                self.pos = BytePos::ZERO;
                self.preferred_col = Some(0);
            }
            CursorMotion::BufferEnd => {
                self.pos = BytePos(buffer.len_bytes());
                self.refresh_preferred_col(buffer)?;
            }
            CursorMotion::Left => {
                if !buffer.is_char_boundary(self.pos) {
                    return Err(CoreError::InvalidCharBoundary { offset: self.pos.0 });
                }
                self.pos = BytePos(prev_grapheme_boundary(&s, self.pos.0));
                self.refresh_preferred_col(buffer)?;
            }
            CursorMotion::Right => {
                if !buffer.is_char_boundary(self.pos) {
                    return Err(CoreError::InvalidCharBoundary { offset: self.pos.0 });
                }
                self.pos = BytePos(next_grapheme_boundary(&s, self.pos.0));
                self.refresh_preferred_col(buffer)?;
            }
            CursorMotion::LineStart => {
                let lc = buffer.byte_to_line_col(self.pos)?;
                let line_start = buffer.line_to_byte(lc.line)?;
                self.pos = BytePos(line_start);
                self.preferred_col = Some(0);
            }
            CursorMotion::LineEnd => {
                let lc = buffer.byte_to_line_col(self.pos)?;
                let line_len = buffer.line_len_bytes(lc.line)?;
                self.pos = buffer.line_col_to_byte(LineCol::new(lc.line, line_len))?;
                self.refresh_preferred_col(buffer)?;
            }
            CursorMotion::Up => self.vertical(buffer, -1)?,
            CursorMotion::Down => self.vertical(buffer, 1)?,
            CursorMotion::WordLeft => {
                let s = buffer.to_text();
                self.pos = BytePos(crate::word_nav::word_left(&s, self.pos.0));
                self.refresh_preferred_col(buffer)?;
            }
            CursorMotion::WordRight => {
                let s = buffer.to_text();
                self.pos = BytePos(crate::word_nav::word_right(&s, self.pos.0));
                self.refresh_preferred_col(buffer)?;
            }
        }
        Ok(())
    }

    fn refresh_preferred_col(&mut self, buffer: &TextBuffer) -> CoreResult<()> {
        let lc = buffer.byte_to_line_col(self.pos)?;
        self.preferred_col = Some(lc.col);
        Ok(())
    }

    fn vertical(&mut self, buffer: &TextBuffer, delta: isize) -> CoreResult<()> {
        let lc = buffer.byte_to_line_col(self.pos)?;
        let target_line = lc.line as isize + delta;
        if target_line < 0 {
            self.pos = BytePos::ZERO;
            self.preferred_col = Some(0);
            return Ok(());
        }
        let total = buffer.len_lines();
        if target_line as usize >= total {
            let last = total.saturating_sub(1);
            let line_len = buffer.line_len_bytes(last)?;
            self.pos = buffer.line_col_to_byte(LineCol::new(last, line_len))?;
            return self.refresh_preferred_col(buffer);
        }
        let line = target_line as usize;
        let line_len = buffer.line_len_bytes(line)?;
        let col = self.preferred_col.unwrap_or(lc.col).min(line_len);
        self.pos = buffer.line_col_to_byte(LineCol::new(line, col))?;
        self.refresh_preferred_col(buffer)
    }
}

fn prev_grapheme_boundary(s: &str, byte: usize) -> usize {
    let byte = byte.min(s.len());
    let mut prev = 0;
    for (idx, _) in s.grapheme_indices(true) {
        if idx >= byte {
            break;
        }
        prev = idx;
    }
    prev
}

fn next_grapheme_boundary(s: &str, byte: usize) -> usize {
    let byte = byte.min(s.len());
    if byte >= s.len() {
        return s.len();
    }
    for (idx, g) in s.grapheme_indices(true) {
        if idx > byte {
            return idx;
        }
        let end = idx + g.len();
        if end > byte {
            return end;
        }
    }
    s.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::TextBuffer;

    #[test]
    fn left_right_ascii() {
        let buf = TextBuffer::from_str("ab");
        let mut c = Cursor::new(BytePos(0));
        c.apply(CursorMotion::Right, &buf).unwrap();
        assert_eq!(c.pos().0, 1);
        c.apply(CursorMotion::Right, &buf).unwrap();
        assert_eq!(c.pos().0, 2);
        c.apply(CursorMotion::Left, &buf).unwrap();
        assert_eq!(c.pos().0, 1);
    }

    #[test]
    fn word_left_right_in_buffer() {
        let b = TextBuffer::from_str("hello world");
        let mut c = Cursor::new(BytePos(b.len_bytes()));
        c.apply(CursorMotion::WordLeft, &b).unwrap();
        assert_eq!(c.pos().0, 6);
        c.apply(CursorMotion::WordLeft, &b).unwrap();
        assert_eq!(c.pos().0, 0);
        c.apply(CursorMotion::WordRight, &b).unwrap();
        assert_eq!(c.pos().0, 5);
    }

    #[test]
    fn grapheme_cluster() {
        let b = TextBuffer::from_str("a👨‍👩‍👧‍👦b"); // allow-emoji: grapheme-cursor fixture
        let mut c = Cursor::new(BytePos(0));
        c.apply(CursorMotion::Right, &b).unwrap();
        assert_eq!(c.pos().0, 1);
        c.apply(CursorMotion::Right, &b).unwrap();
        assert!(c.pos().0 > 1);
        c.apply(CursorMotion::Left, &b).unwrap();
        assert_eq!(c.pos().0, 1);
        c.apply(CursorMotion::Left, &b).unwrap();
        assert_eq!(c.pos().0, 0);
    }
}
