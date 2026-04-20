//! Core error types for the text engine (`editor-core`).

use thiserror::Error;

/// Recoverable errors from pure document operations.
///
/// ```
/// use editor_core::CoreError;
///
/// let err = CoreError::InvalidOffset { offset: 10, len: 5 };
/// assert!(err.to_string().contains("invalid byte offset"));
/// ```
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
    /// Line index out of range for the current line table.
    #[error("invalid line index {line} in document with {total_lines} lines")]
    InvalidLineIndex {
        /// Requested zero-based line index.
        line: usize,
        /// Total number of lines (including the empty trailing line when applicable).
        total_lines: usize,
    },
}

/// Standard result alias for [`CoreError`].
pub type CoreResult<T> = Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctest_invalid_offset_message() {
        let e = CoreError::InvalidOffset {
            offset: 99,
            len: 10,
        };
        assert!(e.to_string().contains("99"));
        assert!(e.to_string().contains("10"));
    }
}
