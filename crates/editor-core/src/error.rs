//! Core error types for the text engine (`editor-core`).

use thiserror::Error;

/// Recoverable errors from pure document operations.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Byte offset out of range for the current document buffer.
    #[error("invalid byte offset {offset} in document of length {len}")]
    InvalidOffset {
        /// Requested offset in bytes.
        offset: usize,
        /// Document length in bytes.
        len: usize,
    },
    /// Byte offset is not on a UTF-8 scalar boundary.
    #[error("byte offset {offset} is not a UTF-8 character boundary")]
    InvalidCharBoundary {
        /// Requested offset in bytes.
        offset: usize,
    },
    /// Line index out of range for the current line table.
    #[error("invalid line index {line} in document with {total_lines} lines")]
    InvalidLineIndex {
        /// Requested zero-based line index.
        line: usize,
        /// Total number of lines (including the empty trailing line when applicable).
        total_lines: usize,
    },
    /// Column is past the end of the requested line.
    #[error("column {col} is past end of line {line} (max column {max_col})")]
    InvalidColumn {
        /// Zero-based line index.
        line: usize,
        /// Requested column in UTF-8 bytes within the line.
        col: usize,
        /// Maximum valid column (byte offset from line start to end of line content).
        max_col: usize,
    },
    /// Invalid byte range (e.g. start > end or not on boundaries).
    #[error("invalid byte range {start}..{end} in document of length {len}")]
    InvalidRange {
        /// Range start in bytes.
        start: usize,
        /// Range end in bytes.
        end: usize,
        /// Document length in bytes.
        len: usize,
    },
    /// Edit could not be applied (e.g. mismatched delete payload).
    #[error("invalid edit: {message}")]
    InvalidEdit {
        /// Static description for logging and tests.
        message: &'static str,
    },
}

/// Standard result alias for [`CoreError`].
pub type CoreResult<T> = Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_offset_message() {
        let e = CoreError::InvalidOffset { offset: 99, len: 10 };
        assert!(e.to_string().contains("99"));
        assert!(e.to_string().contains("10"));
    }
}
