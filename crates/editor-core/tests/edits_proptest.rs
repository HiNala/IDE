//! Property tests for edit apply + inverse round-trips.

use editor_core::{BytePos, EditKind, TextBuffer};
use proptest::prelude::*;

proptest! {
    #[test]
    fn insert_delete_roundtrip(inserts in prop::collection::vec("[a-z]{1,3}", 1..8)) {
        let mut buf = TextBuffer::new();
        let mut pos = 0usize;
        let mut edits = Vec::new();
        for s in &inserts {
            let p = BytePos(pos);
            let e = buf.apply_edit(EditKind::Insert { pos: p, text: s.clone() }).unwrap();
            edits.push(e);
            pos += s.len();
        }
        for e in edits.iter().rev() {
            let inv = e.inverse();
            inv.apply(&mut buf).unwrap();
        }
        prop_assert_eq!(buf.len_bytes(), 0);
    }
}
