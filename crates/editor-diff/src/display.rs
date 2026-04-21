//! Combined inline diff rope + per-line paint metadata for GPU overlays.

use std::ops::Range;

use ropey::Rope;
use similar::TextDiff;

use crate::compute::{diff_op_to_line_ops, merge_delete_insert_to_replace, trim_line};
use crate::types::{CharOp, Hunk, IntraLineDiff, LineOp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffGutter {
    Neutral,
    Insert,
    Delete,
    ReplaceOld,
    ReplaceNew,
}

#[derive(Debug, Clone)]
pub struct InlineDiffLine {
    pub gutter: DiffGutter,
    pub delete_spans: Vec<Range<usize>>,
    pub insert_spans: Vec<Range<usize>>,
}

#[derive(Clone, Copy, Debug)]
pub struct DiffPaint<'a> {
    pub lines: &'a [InlineDiffLine],
}

#[derive(Debug, Clone)]
pub struct InlineDiffDocument {
    pub rope: Rope,
    pub lines: Vec<InlineDiffLine>,
    pub hunk_display_start: Vec<usize>,
    pub hunks: Vec<Hunk>,
}

impl InlineDiffDocument {
    #[must_use]
    pub fn build(old: &str, new: &str) -> Self {
        let hunks = crate::compute_line_diff(old, new);
        let diff = TextDiff::from_lines(old, new);
        let mut groups = diff.grouped_ops(3);
        if groups.is_empty() && !diff.ops().is_empty() {
            groups = vec![diff.ops().to_vec()];
        }
        let old_slices = diff.old_slices();
        let new_slices = diff.new_slices();
        let mut rope = Rope::new();
        let mut lines: Vec<InlineDiffLine> = Vec::new();
        let mut hunk_display_start: Vec<usize> = Vec::new();
        let mut display_i = 0usize;
        for group in groups {
            if group.is_empty() {
                continue;
            }
            hunk_display_start.push(display_i);
            let mut chunk: Vec<LineOp> = Vec::new();
            for op in group {
                chunk.extend(diff_op_to_line_ops(op, old_slices, new_slices));
            }
            let merged = merge_delete_insert_to_replace(&chunk, old_slices, new_slices);
            for op in merged {
                display_i += emit_line_op(&mut rope, &mut lines, op, old_slices, new_slices);
            }
        }
        if rope.len_bytes() > 0 && !rope.to_string().ends_with('\n') {
            rope.insert(rope.len_bytes(), "\n");
        }
        Self { rope, lines, hunk_display_start, hunks }
    }
}

fn push_line(rope: &mut Rope, lines: &mut Vec<InlineDiffLine>, text: &str, meta: InlineDiffLine) {
    if rope.len_bytes() > 0 {
        rope.insert(rope.len_bytes(), "\n");
    }
    rope.insert(rope.len_bytes(), text);
    lines.push(meta);
}

fn spans_from_intra_old(intra: &IntraLineDiff) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    for op in &intra.char_ops {
        if let CharOp::Delete(r) = op {
            out.push(r.clone());
        }
    }
    out
}

fn spans_from_intra_new(intra: &IntraLineDiff) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    for op in &intra.char_ops {
        if let CharOp::Insert(r) = op {
            out.push(r.clone());
        }
    }
    out
}

fn emit_line_op(
    rope: &mut Rope,
    lines: &mut Vec<InlineDiffLine>,
    op: LineOp,
    old_lines: &[&str],
    new_lines: &[&str],
) -> usize {
    match op {
        LineOp::Equal { old_range, .. } => {
            let mut n = 0usize;
            for line in old_lines.iter().take(old_range.end).skip(old_range.start) {
                let t = trim_line(line);
                push_line(
                    rope,
                    lines,
                    t,
                    InlineDiffLine {
                        gutter: DiffGutter::Neutral,
                        delete_spans: vec![],
                        insert_spans: vec![],
                    },
                );
                n += 1;
            }
            n
        }
        LineOp::Delete { old_range } => {
            let mut n = 0usize;
            for line in old_lines.iter().take(old_range.end).skip(old_range.start) {
                let t = trim_line(line);
                push_line(
                    rope,
                    lines,
                    t,
                    InlineDiffLine {
                        gutter: DiffGutter::Delete,
                        delete_spans: vec![],
                        insert_spans: vec![],
                    },
                );
                n += 1;
            }
            n
        }
        LineOp::Insert { new_range } => {
            let mut n = 0usize;
            for line in new_lines.iter().take(new_range.end).skip(new_range.start) {
                let t = trim_line(line);
                push_line(
                    rope,
                    lines,
                    t,
                    InlineDiffLine {
                        gutter: DiffGutter::Insert,
                        delete_spans: vec![],
                        insert_spans: vec![],
                    },
                );
                n += 1;
            }
            n
        }
        LineOp::Replace { old_range, new_range, intra_line } => {
            let mut n = 0usize;
            for (k, intra) in intra_line.iter().enumerate() {
                let oi = old_range.start + k;
                let ni = new_range.start + k;
                let old_t = trim_line(old_lines[oi]);
                push_line(
                    rope,
                    lines,
                    old_t,
                    InlineDiffLine {
                        gutter: DiffGutter::ReplaceOld,
                        delete_spans: spans_from_intra_old(intra),
                        insert_spans: vec![],
                    },
                );
                n += 1;
                let new_t = trim_line(new_lines[ni]);
                push_line(
                    rope,
                    lines,
                    new_t,
                    InlineDiffLine {
                        gutter: DiffGutter::ReplaceNew,
                        delete_spans: vec![],
                        insert_spans: spans_from_intra_new(intra),
                    },
                );
                n += 1;
            }
            n
        }
    }
}
