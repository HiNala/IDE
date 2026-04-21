//! Structural diff types: line ops, character ops, hunks.

use std::ops::Range;

/// One line-level operation inside a [`Hunk`](crate::Hunk).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineOp {
    Equal { old_range: Range<usize>, new_range: Range<usize> },
    Insert { new_range: Range<usize> },
    Delete { old_range: Range<usize> },
    Replace { old_range: Range<usize>, new_range: Range<usize>, intra_line: Vec<IntraLineDiff> },
}

/// Intra-line pairing inside a [`LineOp::Replace`] block (one old/new line pair).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntraLineDiff {
    /// Index into the Replace hunk’s `old_range` (0 = first old line).
    pub old_line_idx: usize,
    /// Index into the Replace hunk’s `new_range`.
    pub new_line_idx: usize,
    pub char_ops: Vec<CharOp>,
}

/// Character-level span in an old or new line (UTF-8 byte indices within that line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharOp {
    Equal(Range<usize>),
    Insert(Range<usize>),
    Delete(Range<usize>),
}

/// Unified-diff-style hunk header (1-based line numbers, inclusive counts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HunkHeader {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
}

/// One grouped hunk (nearby line-level changes merged with context radius).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub header: HunkHeader,
    pub ops: Vec<LineOp>,
}
