//! Line-level diff grouping on top of [`similar::TextDiff`].

use similar::{DiffOp, TextDiff};

use crate::intra_line::compute_intra_line_diff;
use crate::types::{Hunk, HunkHeader, IntraLineDiff, LineOp};

fn hunk_header_for_ops(ops: &[DiffOp]) -> HunkHeader {
    let mut old_start = None::<usize>;
    let mut new_start = None::<usize>;
    let mut old_lines = 0usize;
    let mut new_lines = 0usize;
    for op in ops {
        match *op {
            DiffOp::Equal { old_index, new_index, len } => {
                if old_start.is_none() {
                    old_start = Some(old_index);
                }
                if new_start.is_none() {
                    new_start = Some(new_index);
                }
                old_lines += len;
                new_lines += len;
            }
            DiffOp::Delete { old_index, old_len, .. } => {
                old_start.get_or_insert(old_index);
                old_lines += old_len;
            }
            DiffOp::Insert { new_index, new_len, .. } => {
                new_start.get_or_insert(new_index);
                new_lines += new_len;
            }
            DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                old_start.get_or_insert(old_index);
                new_start.get_or_insert(new_index);
                old_lines += old_len;
                new_lines += new_len;
            }
        }
    }
    HunkHeader {
        old_start: old_start.map(|i| i.saturating_add(1)).unwrap_or(1),
        old_lines,
        new_start: new_start.map(|i| i.saturating_add(1)).unwrap_or(1),
        new_lines,
    }
}

pub(crate) fn trim_line(s: &str) -> &str {
    s.trim_end_matches(['\n', '\r'])
}

pub(crate) fn diff_op_to_line_ops(
    op: DiffOp,
    old_lines: &[&str],
    new_lines: &[&str],
) -> Vec<LineOp> {
    match op {
        DiffOp::Equal { old_index, new_index, len } => {
            vec![LineOp::Equal {
                old_range: old_index..old_index + len,
                new_range: new_index..new_index + len,
            }]
        }
        DiffOp::Delete { old_index, old_len, .. } => {
            vec![LineOp::Delete { old_range: old_index..old_index + old_len }]
        }
        DiffOp::Insert { new_index, new_len, .. } => {
            vec![LineOp::Insert { new_range: new_index..new_index + new_len }]
        }
        DiffOp::Replace { old_index, old_len, new_index, new_len } => {
            let pairs = old_len.min(new_len);
            let mut out = Vec::new();
            if pairs > 0 {
                let mut intra = Vec::with_capacity(pairs);
                for k in 0..pairs {
                    let o_line = trim_line(old_lines[old_index + k]);
                    let n_line = trim_line(new_lines[new_index + k]);
                    intra.push(IntraLineDiff {
                        old_line_idx: k,
                        new_line_idx: k,
                        char_ops: compute_intra_line_diff(o_line, n_line),
                    });
                }
                out.push(LineOp::Replace {
                    old_range: old_index..old_index + pairs,
                    new_range: new_index..new_index + pairs,
                    intra_line: intra,
                });
            }
            if old_len > pairs {
                out.push(LineOp::Delete { old_range: old_index + pairs..old_index + old_len });
            }
            if new_len > pairs {
                out.push(LineOp::Insert { new_range: new_index + pairs..new_index + new_len });
            }
            out
        }
    }
}

/// Line- and character-level diff grouped into unified-style hunks (3 lines of context).
#[must_use]
pub fn compute_line_diff(old: &str, new: &str) -> Vec<Hunk> {
    let diff = TextDiff::from_lines(old, new);
    let old_slices = diff.old_slices();
    let new_slices = diff.new_slices();

    let mut groups = diff.grouped_ops(3);
    if groups.is_empty() {
        if diff.ops().is_empty() {
            return Vec::new();
        }
        groups = vec![diff.ops().to_vec()];
    }

    let mut hunks = Vec::with_capacity(groups.len());
    for group in groups {
        if group.is_empty() {
            continue;
        }
        let header = hunk_header_for_ops(&group);
        let mut ops = Vec::new();
        for op in group {
            ops.extend(diff_op_to_line_ops(op, old_slices, new_slices));
        }
        let ops = merge_delete_insert_to_replace(&ops, old_slices, new_slices);
        hunks.push(Hunk { header, ops });
    }
    hunks
}

/// Full-file [`LineOp`] sequence (used to build a single combined inline view).
#[must_use]
#[allow(dead_code)] // used by integration tests and future single-hunk UI; keep exported
pub fn flatten_line_ops(old: &str, new: &str) -> Vec<LineOp> {
    let diff = TextDiff::from_lines(old, new);
    let old_slices = diff.old_slices();
    let new_slices = diff.new_slices();
    let mut out = Vec::new();
    for op in diff.ops() {
        out.extend(diff_op_to_line_ops(*op, old_slices, new_slices));
    }
    merge_delete_insert_to_replace(&out, old_slices, new_slices)
}

pub(crate) fn merge_delete_insert_to_replace(
    ops: &[LineOp],
    old_lines: &[&str],
    new_lines: &[&str],
) -> Vec<LineOp> {
    let mut out: Vec<LineOp> = Vec::new();
    let mut i = 0;
    while i < ops.len() {
        if let (LineOp::Delete { old_range }, Some(LineOp::Insert { new_range })) =
            (&ops[i], ops.get(i + 1))
        {
            let old_n = old_range.end.saturating_sub(old_range.start);
            let new_n = new_range.end.saturating_sub(new_range.start);
            if old_n > 0 && new_n > 0 {
                let pairs = old_n.min(new_n);
                let mut intra = Vec::with_capacity(pairs);
                for k in 0..pairs {
                    let o_line = trim_line(old_lines[old_range.start + k]);
                    let n_line = trim_line(new_lines[new_range.start + k]);
                    intra.push(IntraLineDiff {
                        old_line_idx: k,
                        new_line_idx: k,
                        char_ops: compute_intra_line_diff(o_line, n_line),
                    });
                }
                out.push(LineOp::Replace {
                    old_range: old_range.start..old_range.start + pairs,
                    new_range: new_range.start..new_range.start + pairs,
                    intra_line: intra,
                });
                if old_n > pairs {
                    out.push(LineOp::Delete { old_range: old_range.start + pairs..old_range.end });
                }
                if new_n > pairs {
                    out.push(LineOp::Insert { new_range: new_range.start + pairs..new_range.end });
                }
                i += 2;
                continue;
            }
        }
        out.push(ops[i].clone());
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LineOp;

    #[test]
    fn identical_single_equal_hunk() {
        let h = compute_line_diff("a\nb\n", "a\nb\n");
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].ops.len(), 1);
        assert!(matches!(h[0].ops[0], LineOp::Equal { .. }));
    }

    #[test]
    fn replace_has_intra_line() {
        let h = compute_line_diff("foo\n", "bar\n");
        let replace = h.iter().flat_map(|x| &x.ops).find_map(|op| match op {
            LineOp::Replace { intra_line, .. } => Some(intra_line),
            _ => None,
        });
        assert!(replace.is_some_and(|v| !v.is_empty()));
    }

    #[test]
    fn flatten_line_ops_smoke() {
        let ops = flatten_line_ops("a\n", "b\n");
        assert!(!ops.is_empty());
    }
}
