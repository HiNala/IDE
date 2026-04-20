//! Property-based checks for buffer and position invariants.

use editor_core::{BytePos, TextBuffer};
use proptest::prelude::*;

proptest! {
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
}

#[test]
fn delete_insert_roundtrip_small() {
    let mut buf = TextBuffer::from_str("hello world");
    let deleted = buf.delete_range(BytePos(5)..BytePos(6)).unwrap();
    assert_eq!(deleted, " ");
    buf.insert(BytePos(5), " ").unwrap();
    assert_eq!(buf.to_text(), "hello world");
}
