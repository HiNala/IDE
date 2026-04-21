//! `editor-core` — pure text engine: rope buffer, cursor, selection, undo/redo.
//!
//! Byte offsets ([`BytePos`]) are canonical; line/column are derived via
//! [`TextBuffer::byte_to_line_col`] / [`TextBuffer::line_col_to_byte`].
//!
//! # Example
//!
//! ```
//! use editor_core::{
//!     BytePos, EditKind, TextBuffer, UndoStack,
//! };
//!
//! let mut buf = TextBuffer::from_str("hi\n");
//! let mut undo = UndoStack::default();
//! let edit = buf
//!     .apply_edit(EditKind::Insert {
//!         pos: BytePos(0),
//!         text: "x".into(),
//!     })
//!     .unwrap();
//! undo.push(edit);
//! assert_eq!(buf.to_text(), "xhi\n");
//! undo.undo(&mut buf).unwrap();
//! assert_eq!(buf.to_text(), "hi\n");
//! undo.redo(&mut buf).unwrap();
//! assert_eq!(buf.to_text(), "xhi\n");
//! ```

#![forbid(unsafe_code)]

pub mod buffer;
pub mod cursor;
pub mod error;
pub mod position;
pub mod scroll;
pub mod selection;
pub mod undo;
pub mod word_nav;
pub mod worker;

pub use buffer::line_ending::LineEnding;
pub use buffer::{Edit, EditKind, TextBuffer, TextBufferSnapshot};
pub use cursor::{Cursor, CursorMotion};
pub use error::{CoreError, CoreResult};
pub use position::{BytePos, LineCol};
pub use scroll::ScrollOffset;
pub use selection::Selection;
pub use undo::UndoStack;
pub use word_nav::{delete_word_backward_range, delete_word_forward_range, word_left, word_right};
pub use worker::{JobToken, WorkerPool};

/// Crate version string, sourced from `Cargo.toml` at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns a human-readable banner identifying this crate.
#[must_use]
pub fn banner() -> String {
    format!("editor-core v{VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_crate_name_and_version() {
        let b = banner();
        assert!(b.starts_with("editor-core v"), "banner = {b:?}");
        assert!(b.contains(VERSION), "banner = {b:?}");
    }
}
