//! Vertical diff marks in the editor gutter (M17 + M18).
//!
//! Converts a list of [`editor_diff::Hunk`]s into per-line state and paints
//! a thin colored stripe at the left edge of the gutter so git-modified
//! lines become visible at a glance:
//!
//! * **green**  — newly inserted lines (no counterpart in HEAD)
//! * **yellow** — lines inside a `Replace` block (content changed)
//! * **red triangle** — a deletion marker between two equal lines
//!
//! The painter is purely additive: if there are no hunks, no quads are
//! pushed. Safe to call every frame.

use std::ops::Range;

use editor_diff::{Hunk, LineOp};

use crate::chrome::{ChromeQuad, FrameChrome};
use crate::theme::palette;

/// Per-line change kind used by the gutter painter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GutterMark {
    /// New line inserted relative to HEAD (paint green).
    Added,
    /// Line content changed relative to HEAD (paint yellow).
    Modified,
    /// Marker for a deletion that happened *before* this line (paint red).
    /// Painted as a small triangle sitting on the top edge of the row.
    DeletedAbove,
}

/// Reduce hunks to a `total_lines`-sized Vec of optional marks, indexed by
/// **new** (worktree) line number (0-based).
#[must_use]
pub fn compute_gutter_marks(hunks: &[Hunk], total_lines: usize) -> Vec<Option<GutterMark>> {
    let mut marks = vec![None; total_lines];
    for hunk in hunks {
        for op in &hunk.ops {
            match op {
                LineOp::Equal { .. } => {}
                LineOp::Insert { new_range } => {
                    fill(&mut marks, new_range.clone(), GutterMark::Added);
                }
                LineOp::Replace { new_range, .. } => {
                    fill(&mut marks, new_range.clone(), GutterMark::Modified);
                }
                LineOp::Delete { old_range: _ } => {
                    // Line(s) removed — paint a "deleted above" marker on the
                    // first new line that survived (i.e., the Equal block that
                    // follows this Delete in the hunk). The hunk header's
                    // new_start + the number of new lines painted so far gives
                    // us that row. Since we can't easily walk "forward" inside
                    // an iter, mark based on the hunk header's new line where
                    // the deletion lands.
                    let marker = hunk.header.new_start.saturating_sub(1);
                    if marker < marks.len() {
                        // Preserve any stronger mark (Added / Modified) already
                        // present — "deleted above" is strictly informational.
                        if marks[marker].is_none() {
                            marks[marker] = Some(GutterMark::DeletedAbove);
                        }
                    }
                }
            }
        }
    }
    marks
}

fn fill(marks: &mut [Option<GutterMark>], range: Range<usize>, mark: GutterMark) {
    for i in range {
        if i < marks.len() {
            marks[i] = Some(mark);
        }
    }
}

/// Paint one mark per visible row. `visible_lines` is the `[first, last)`
/// row range; `row_height_px` and `row_top_px` are in physical pixels (with
/// `row_top_px` matching the top of the *first* visible row, already
/// accounting for scroll offset).
pub fn paint_gutter_marks(
    chrome: &mut FrameChrome,
    marks: &[Option<GutterMark>],
    visible_lines: Range<usize>,
    gutter_left_px: f32,
    row_top_px: f32,
    row_height_px: f32,
    scale: f32,
) {
    if row_height_px <= 0.0 {
        return;
    }
    let stripe_w = (2.5 * scale).max(1.0);
    let tri_size = (6.0 * scale).max(2.0);
    for (i, line_idx) in visible_lines.clone().enumerate() {
        let Some(mark) = marks.get(line_idx).and_then(|m| *m) else {
            continue;
        };
        let top = row_top_px + i as f32 * row_height_px;
        match mark {
            GutterMark::Added => {
                chrome.push_quad(ChromeQuad {
                    left: gutter_left_px,
                    top,
                    width: stripe_w,
                    height: row_height_px,
                    rgba: palette::DIFF_ADDED,
                });
            }
            GutterMark::Modified => {
                chrome.push_quad(ChromeQuad {
                    left: gutter_left_px,
                    top,
                    width: stripe_w,
                    height: row_height_px,
                    rgba: palette::DIFF_MODIFIED,
                });
            }
            GutterMark::DeletedAbove => {
                // Tiny filled square sitting on the top edge of the row.
                chrome.push_quad(ChromeQuad {
                    left: gutter_left_px,
                    top: top - tri_size / 2.0,
                    width: tri_size,
                    height: tri_size,
                    rgba: palette::DIFF_REMOVED,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use editor_diff::{Hunk, HunkHeader, LineOp};

    fn eq(old: Range<usize>, new: Range<usize>) -> LineOp {
        LineOp::Equal { old_range: old, new_range: new }
    }

    #[test]
    fn compute_returns_vec_sized_to_total_lines() {
        let marks = compute_gutter_marks(&[], 5);
        assert_eq!(marks.len(), 5);
        assert!(marks.iter().all(Option::is_none));
    }

    #[test]
    fn insert_marks_added_lines() {
        let hunk = Hunk {
            header: HunkHeader { old_start: 1, old_lines: 0, new_start: 3, new_lines: 2 },
            ops: vec![eq(0..2, 0..2), LineOp::Insert { new_range: 2..4 }, eq(2..3, 4..5)],
        };
        let marks = compute_gutter_marks(&[hunk], 5);
        assert_eq!(marks[0], None);
        assert_eq!(marks[1], None);
        assert_eq!(marks[2], Some(GutterMark::Added));
        assert_eq!(marks[3], Some(GutterMark::Added));
        assert_eq!(marks[4], None);
    }

    #[test]
    fn replace_marks_modified_lines() {
        let hunk = Hunk {
            header: HunkHeader { old_start: 1, old_lines: 1, new_start: 1, new_lines: 1 },
            ops: vec![LineOp::Replace { old_range: 0..1, new_range: 0..1, intra_line: Vec::new() }],
        };
        let marks = compute_gutter_marks(&[hunk], 3);
        assert_eq!(marks[0], Some(GutterMark::Modified));
        assert_eq!(marks[1], None);
    }

    #[test]
    fn delete_drops_a_marker_above_the_affected_row() {
        let hunk = Hunk {
            header: HunkHeader { old_start: 3, old_lines: 1, new_start: 3, new_lines: 0 },
            ops: vec![eq(0..2, 0..2), LineOp::Delete { old_range: 2..3 }, eq(3..4, 2..3)],
        };
        let marks = compute_gutter_marks(&[hunk], 4);
        // new_start is 3 (1-based) → marker at new line 2 (0-based).
        assert_eq!(marks[2], Some(GutterMark::DeletedAbove));
    }

    #[test]
    fn paint_is_noop_when_no_marks_visible() {
        let mut c = FrameChrome::new();
        let marks: Vec<Option<GutterMark>> = vec![None, None, None];
        paint_gutter_marks(&mut c, &marks, 0..3, 0.0, 0.0, 20.0, 1.0);
        assert!(c.quads.is_empty());
    }

    #[test]
    fn paint_emits_one_quad_per_marked_visible_row() {
        let mut c = FrameChrome::new();
        let marks = vec![
            Some(GutterMark::Added),
            None,
            Some(GutterMark::Modified),
            Some(GutterMark::DeletedAbove),
        ];
        paint_gutter_marks(&mut c, &marks, 0..4, 4.0, 0.0, 20.0, 1.0);
        assert_eq!(c.quads.len(), 3);
    }

    #[test]
    fn paint_respects_visible_range() {
        let mut c = FrameChrome::new();
        let marks = vec![Some(GutterMark::Added); 10];
        // Only 3 rows visible — we expect 3 quads regardless of doc size.
        paint_gutter_marks(&mut c, &marks, 4..7, 0.0, 0.0, 20.0, 1.0);
        assert_eq!(c.quads.len(), 3);
    }
}
