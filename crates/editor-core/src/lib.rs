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
//! assert!(buf.slice_to_string(BytePos(0)..BytePos(1)).unwrap() == "x");
//! undo.undo(&mut buf).unwrap();
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

pub use buffer::line_ending::LineEnding;
pub use buffer::{Edit, EditKind, TextBuffer, TextBufferSnapshot};
pub use cursor::{Cursor, CursorMotion};
pub use error::{CoreError, CoreResult};
pub use position::{BytePos, LineCol};
pub use scroll::ScrollOffset;
pub use selection::Selection;
pub use undo::UndoStack;
pub use word_nav::{delete_word_backward_range, delete_word_forward_range, word_left, word_right};

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
