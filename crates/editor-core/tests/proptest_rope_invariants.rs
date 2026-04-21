//! Property-based checks for buffer and position invariants.
//!
//! Runs at least **256 cases** per `proptest!` block (M02 acceptance).

use std::time::Duration;

use editor_core::{BytePos, Cursor, CursorMotion, EditKind, TextBuffer, UndoStack};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        ..ProptestConfig::default()
    })]

    #[test]
    fn insert_at_start_matches_len(chunks in prop::collection::vec(prop::string::string_regex(".{0,32}").unwrap(), 1..8)) {
        let mut buf = TextBuffer::new();
        let mut total = 0usize;
        for part in &chunks {
            buf.insert(BytePos(total), part).unwrap();
            total += part.len();
        }
        prop_assert_eq!(buf.len_bytes(), total);
    }

    #[test]
    fn byte_line_col_roundtrip_on_single_line(s in prop::string::string_regex("[a-z]{0,40}").unwrap()) {
        let doc = format!("{s}\n");
        let buf = TextBuffer::from_str(&doc);
        let len = buf.len_bytes();
        for byte in (0..=len).filter(|&b| buf.is_char_boundary(BytePos(b))) {
            let lc = buf.byte_to_line_col(BytePos(byte)).unwrap();
            let back = buf.line_col_to_byte(lc).unwrap();
            prop_assert_eq!(back.0, byte);
        }
    }

    /// After a bounded random motion sequence, the caret stays on a UTF-8 boundary when moves succeed.
    #[test]
    fn cursor_on_char_boundary_ascii(
        s in prop::string::string_regex("[a-z\n]{0,80}").unwrap(),
        motions in prop::collection::vec(0u8..8, 1..48)
    ) {
        let buf = TextBuffer::from_str(&s);
        let mut c = Cursor::new(BytePos(0));
        for code in motions {
            let motion = match code % 8 {
                0 => CursorMotion::Left,
                1 => CursorMotion::Right,
                2 => CursorMotion::Up,
                3 => CursorMotion::Down,
                4 => CursorMotion::LineStart,
                5 => CursorMotion::LineEnd,
                6 => CursorMotion::BufferStart,
                _ => CursorMotion::BufferEnd,
            };
            if c.apply(motion, &buf).is_ok() {
                prop_assert!(buf.is_char_boundary(c.pos()), "pos={}", c.pos().0);
            }
        }
    }

    /// With [`UndoStack::checkpoint`] after each edit, N undos then N redos restores the buffer text.
    #[test]
    fn undo_checkpoint_then_redo_roundtrip(n in 1usize..24usize) {
        let mut buf = TextBuffer::new();
        let mut u = UndoStack::new(256, Duration::from_secs(10));
        for _ in 0..n {
            let pos = BytePos(buf.len_bytes());
            let e = buf
                .apply_edit(EditKind::Insert { pos, text: "x".into() })
                .unwrap();
            u.push(e);
            u.checkpoint();
        }
        let expected = buf.to_text();
        for _ in 0..n {
            u.undo(&mut buf).unwrap();
        }
        prop_assert_eq!(buf.len_bytes(), 0);
        for _ in 0..n {
            u.redo(&mut buf).unwrap();
        }
        prop_assert_eq!(buf.to_text(), expected);
    }
}

#[test]
fn delete_insert_roundtrip_small() {
    let mut buf = TextBuffer::from_str("hello world");
    let deleted = buf.delete_range(BytePos(5)..BytePos(6)).unwrap();
    assert_eq!(deleted, " ");
    buf.insert(BytePos(5), " ").unwrap();
    assert_eq!(buf.to_text(), "hello world");
}
