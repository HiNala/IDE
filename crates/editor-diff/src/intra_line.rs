//! Character-level diff with a similarity fallback (noise reduction).

use similar::{ChangeTag, TextDiff};

use crate::types::CharOp;

/// Character-level diff for one line pair. Falls back to whole-line delete+insert when
/// [`TextDiff::ratio`](similar::TextDiff::ratio) is below 30%.
#[must_use]
pub fn compute_intra_line_diff(old_line: &str, new_line: &str) -> Vec<CharOp> {
    let diff = TextDiff::from_chars(old_line, new_line);
    if diff.ratio() < 0.3 {
        let mut v = Vec::new();
        if !old_line.is_empty() {
            v.push(CharOp::Delete(0..old_line.len()));
        }
        if !new_line.is_empty() {
            v.push(CharOp::Insert(0..new_line.len()));
        }
        return v;
    }

    let mut out = Vec::new();
    let mut ob = 0usize;
    let mut nb = 0usize;
    for c in diff.iter_all_changes() {
        let v = c.value_ref();
        let len = v.len();
        match c.tag() {
            ChangeTag::Equal => {
                if len > 0 {
                    out.push(CharOp::Equal(ob..ob + len));
                }
                ob += len;
                nb += len;
            }
            ChangeTag::Delete => {
                if len > 0 {
                    out.push(CharOp::Delete(ob..ob + len));
                }
                ob += len;
            }
            ChangeTag::Insert => {
                if len > 0 {
                    out.push(CharOp::Insert(nb..nb + len));
                }
                nb += len;
            }
        }
    }
    merge_adjacent_char_ops(out)
}

fn merge_adjacent_char_ops(ops: Vec<CharOp>) -> Vec<CharOp> {
    let mut out: Vec<CharOp> = Vec::new();
    for op in ops {
        match (out.last_mut(), &op) {
            (Some(CharOp::Equal(a)), CharOp::Equal(b)) if a.end == b.start => {
                a.end = b.end;
            }
            (Some(CharOp::Insert(a)), CharOp::Insert(b)) if a.end == b.start => {
                a.end = b.end;
            }
            (Some(CharOp::Delete(a)), CharOp::Delete(b)) if a.end == b.start => {
                a.end = b.end;
            }
            _ => out.push(op),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_lines_one_equal() {
        let ops = compute_intra_line_diff("foo", "foo");
        assert_eq!(ops, vec![CharOp::Equal(0..3)]);
    }

    #[test]
    fn tiny_change() {
        let ops = compute_intra_line_diff("hello", "hallo");
        assert!(!ops.is_empty());
        assert!(ops.iter().any(|o| matches!(o, CharOp::Equal(_))));
    }
}
